package main

import "fmt"

type BaseRegistry struct {
	functions  map[string]func(args map[string]interface{}, ctx *Context) (interface{}, error)
	assertions map[string]func(params map[string]interface{}, ctx *Context) AssertionResult
	hooks      map[string]func(ctx *Context) error
}

func NewBaseRegistry() *BaseRegistry {
	return &BaseRegistry{
		functions:  make(map[string]func(args map[string]interface{}, ctx *Context) (interface{}, error)),
		assertions: make(map[string]func(params map[string]interface{}, ctx *Context) AssertionResult),
		hooks:      make(map[string]func(ctx *Context) error),
	}
}

func (r *BaseRegistry) RegisterFunction(name string, fn func(args map[string]interface{}, ctx *Context) (interface{}, error)) {
	r.functions[name] = fn
}

func (r *BaseRegistry) RegisterAssertion(name string, fn func(params map[string]interface{}, ctx *Context) AssertionResult) {
	r.assertions[name] = fn
}

func (r *BaseRegistry) RegisterHook(name string, fn func(ctx *Context) error) {
	r.hooks[name] = fn
}

func (r *BaseRegistry) Call(name string, args map[string]interface{}, ctx *Context) (interface{}, error) {
	fn, ok := r.functions[name]
	if !ok {
		available := make([]string, 0, len(r.functions))
		for k := range r.functions {
			available = append(available, k)
		}
		return nil, fmt.Errorf("function not found: %s. Available: %v", name, available)
	}
	return fn(args, ctx)
}

func (r *BaseRegistry) ListFunctions() []FunctionInfo {
	functions := make([]FunctionInfo, 0, len(r.functions))
	for name := range r.functions {
		functions = append(functions, FunctionInfo{Name: name})
	}
	return functions
}

func (r *BaseRegistry) CallAssertion(name string, params map[string]interface{}, ctx *Context) AssertionResult {
	fn, ok := r.assertions[name]
	if !ok {
		available := make([]string, 0, len(r.assertions))
		for k := range r.assertions {
			available = append(available, k)
		}
		return AssertionResult{
			Success: false,
			Message: fmt.Sprintf("assertion not found: %s. Available: %v", name, available),
		}
	}
	return fn(params, ctx)
}

func (r *BaseRegistry) CallHook(hook string, ctx *Context) error {
	fn, ok := r.hooks[hook]
	if !ok {
		return nil
	}
	return fn(ctx)
}
