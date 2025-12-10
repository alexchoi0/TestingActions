package main

import (
	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"plugin"
	"strings"
	"sync"
)

type ClockState struct {
	VirtualTimeMs  *int64  `json:"virtual_time_ms"`
	VirtualTimeIso *string `json:"virtual_time_iso"`
	Frozen         bool    `json:"frozen"`
}

type Context struct {
	data     map[string]interface{}
	steps    map[string]map[string]interface{}
	RunID    string
	JobName  string
	StepName string
	Clock    *ClockState
	mu       sync.RWMutex
}

func NewContext() *Context {
	return &Context{
		data:  make(map[string]interface{}),
		steps: make(map[string]map[string]interface{}),
	}
}

func (c *Context) Get(key string) interface{} {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.data[key]
}

func (c *Context) Set(key string, value interface{}) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.data[key] = value
}

func (c *Context) Remove(key string) bool {
	c.mu.Lock()
	defer c.mu.Unlock()
	if _, exists := c.data[key]; exists {
		delete(c.data, key)
		return true
	}
	return false
}

func (c *Context) Clear(pattern string) int {
	c.mu.Lock()
	defer c.mu.Unlock()
	count := 0
	for key := range c.data {
		if matchPattern(pattern, key) {
			delete(c.data, key)
			count++
		}
	}
	return count
}

func (c *Context) GetStepOutput(stepID, outputName string) interface{} {
	c.mu.RLock()
	defer c.mu.RUnlock()
	if step, ok := c.steps[stepID]; ok {
		if outputs, ok := step["outputs"].(map[string]interface{}); ok {
			return outputs[outputName]
		}
	}
	return nil
}

func matchPattern(pattern, s string) bool {
	if pattern == "*" {
		return true
	}
	if strings.HasSuffix(pattern, "*") {
		return strings.HasPrefix(s, pattern[:len(pattern)-1])
	}
	if strings.HasPrefix(pattern, "*") {
		return strings.HasSuffix(s, pattern[1:])
	}
	return pattern == s
}

type FunctionInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
}

type AssertionResult struct {
	Success  bool        `json:"success"`
	Message  string      `json:"message,omitempty"`
	Actual   interface{} `json:"actual,omitempty"`
	Expected interface{} `json:"expected,omitempty"`
}

type Registry interface {
	Call(name string, args map[string]interface{}, ctx *Context) (interface{}, error)
	ListFunctions() []FunctionInfo
	CallAssertion(name string, params map[string]interface{}, ctx *Context) AssertionResult
	CallHook(hook string, ctx *Context) error
}

type JSONRPCRequest struct {
	JSONRPC string                 `json:"jsonrpc"`
	ID      interface{}            `json:"id"`
	Method  string                 `json:"method"`
	Params  map[string]interface{} `json:"params"`
}

type JSONRPCResponse struct {
	JSONRPC string      `json:"jsonrpc"`
	ID      interface{} `json:"id"`
	Result  interface{} `json:"result,omitempty"`
	Error   *RPCError   `json:"error,omitempty"`
}

type RPCError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

func jsonRPCSuccess(id interface{}, result interface{}) JSONRPCResponse {
	return JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      id,
		Result:  result,
	}
}

func jsonRPCError(id interface{}, code int, message string) JSONRPCResponse {
	return JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      id,
		Error:   &RPCError{Code: code, Message: message},
	}
}

type Server struct {
	registry Registry
	ctx      *Context
}

func NewServer(registry Registry) *Server {
	return &Server{
		registry: registry,
		ctx:      NewContext(),
	}
}

func (s *Server) handleFnCall(params map[string]interface{}) (interface{}, error) {
	name, _ := params["name"].(string)
	args, _ := params["args"].(map[string]interface{})
	if args == nil {
		args = make(map[string]interface{})
	}

	result, err := s.registry.Call(name, args, s.ctx)
	if err != nil {
		return nil, err
	}
	return map[string]interface{}{"result": result}, nil
}

func (s *Server) handleCtxGet(params map[string]interface{}) (interface{}, error) {
	key, _ := params["key"].(string)
	return map[string]interface{}{"value": s.ctx.Get(key)}, nil
}

func (s *Server) handleCtxSet(params map[string]interface{}) (interface{}, error) {
	key, _ := params["key"].(string)
	value := params["value"]
	s.ctx.Set(key, value)
	return map[string]interface{}{}, nil
}

func (s *Server) handleCtxClear(params map[string]interface{}) (interface{}, error) {
	pattern, _ := params["pattern"].(string)
	if pattern == "" {
		pattern = "*"
	}
	cleared := s.ctx.Clear(pattern)
	return map[string]interface{}{"cleared": cleared}, nil
}

func (s *Server) handleCtxSetExecutionInfo(params map[string]interface{}) (interface{}, error) {
	s.ctx.RunID, _ = params["runId"].(string)
	s.ctx.JobName, _ = params["jobName"].(string)
	s.ctx.StepName, _ = params["stepName"].(string)
	return map[string]interface{}{}, nil
}

func (s *Server) handleCtxSyncStepOutputs(params map[string]interface{}) (interface{}, error) {
	stepID, _ := params["stepId"].(string)
	outputs, _ := params["outputs"].(map[string]interface{})

	s.ctx.mu.Lock()
	defer s.ctx.mu.Unlock()

	if _, ok := s.ctx.steps[stepID]; !ok {
		s.ctx.steps[stepID] = make(map[string]interface{})
	}
	s.ctx.steps[stepID]["outputs"] = outputs
	return map[string]interface{}{}, nil
}

func (s *Server) handleHookCall(params map[string]interface{}) (interface{}, error) {
	hook, _ := params["hook"].(string)
	err := s.registry.CallHook(hook, s.ctx)
	if err != nil {
		return nil, err
	}
	return map[string]interface{}{}, nil
}

func (s *Server) handleAssertCustom(params map[string]interface{}) (interface{}, error) {
	name, _ := params["name"].(string)
	assertParams, _ := params["params"].(map[string]interface{})
	if assertParams == nil {
		assertParams = make(map[string]interface{})
	}

	result := s.registry.CallAssertion(name, assertParams, s.ctx)
	return result, nil
}

func (s *Server) handleListFunctions(params map[string]interface{}) (interface{}, error) {
	functions := s.registry.ListFunctions()
	return map[string]interface{}{"functions": functions}, nil
}

func (s *Server) handleClockSync(params map[string]interface{}) (interface{}, error) {
	var virtualTimeMs *int64
	var virtualTimeIso *string

	if v, ok := params["virtual_time_ms"].(float64); ok {
		ms := int64(v)
		virtualTimeMs = &ms
	}
	if v, ok := params["virtual_time_iso"].(string); ok {
		virtualTimeIso = &v
	}
	frozen, _ := params["frozen"].(bool)

	s.ctx.Clock = &ClockState{
		VirtualTimeMs:  virtualTimeMs,
		VirtualTimeIso: virtualTimeIso,
		Frozen:         frozen,
	}
	return map[string]interface{}{}, nil
}

func (s *Server) Run() {
	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 1024*1024), 1024*1024)

	fmt.Fprintln(os.Stderr, "Go bridge server started")

	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			continue
		}

		var request JSONRPCRequest
		if err := json.Unmarshal([]byte(line), &request); err != nil {
			fmt.Fprintf(os.Stderr, "Invalid JSON: %s\n", line)
			continue
		}

		var response JSONRPCResponse

		switch request.Method {
		case "fn.call":
			result, err := s.handleFnCall(request.Params)
			if err != nil {
				response = jsonRPCError(request.ID, -32000, err.Error())
			} else {
				response = jsonRPCSuccess(request.ID, result)
			}
		case "ctx.get":
			result, _ := s.handleCtxGet(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "ctx.set":
			result, _ := s.handleCtxSet(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "ctx.clear":
			result, _ := s.handleCtxClear(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "ctx.setExecutionInfo":
			result, _ := s.handleCtxSetExecutionInfo(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "ctx.syncStepOutputs":
			result, _ := s.handleCtxSyncStepOutputs(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "hook.call":
			result, err := s.handleHookCall(request.Params)
			if err != nil {
				response = jsonRPCError(request.ID, -32000, err.Error())
			} else {
				response = jsonRPCSuccess(request.ID, result)
			}
		case "assert.custom":
			result, _ := s.handleAssertCustom(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "list_functions":
			result, _ := s.handleListFunctions(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		case "clock.sync":
			result, _ := s.handleClockSync(request.Params)
			response = jsonRPCSuccess(request.ID, result)
		default:
			response = jsonRPCError(request.ID, -32601, fmt.Sprintf("Method not found: %s", request.Method))
		}

		responseJSON, _ := json.Marshal(response)
		fmt.Println(string(responseJSON))
	}
}

func Serve(registry Registry) {
	server := NewServer(registry)
	server.Run()
}

func main() {
	pluginPath := flag.String("plugin", "", "Path to the Go plugin (.so file)")
	flag.Parse()

	if *pluginPath == "" {
		fmt.Fprintln(os.Stderr, "Usage: server --plugin path/to/registry.so")
		os.Exit(1)
	}

	p, err := plugin.Open(*pluginPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to open plugin: %v\n", err)
		os.Exit(1)
	}

	sym, err := p.Lookup("Registry")
	if err != nil {
		fmt.Fprintf(os.Stderr, "Plugin must export 'Registry' variable: %v\n", err)
		os.Exit(1)
	}

	registry, ok := sym.(*Registry)
	if !ok {
		fmt.Fprintln(os.Stderr, "Registry must implement the Registry interface")
		os.Exit(1)
	}

	Serve(*registry)
}
