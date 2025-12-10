// Example Registry for Go Bridge
//
// This file demonstrates how to create a Go registry that can be loaded
// as a plugin by the Go bridge server.
//
// Build as a plugin:
//   go build -buildmode=plugin -o registry.so example_registry.go
//
// Or embed directly in a custom main:
//   package main
//   func main() { Serve(createRegistry()) }

package main

import (
	"fmt"
	"os"
	"time"
)

func createExampleRegistry() *BaseRegistry {
	r := NewBaseRegistry()

	r.RegisterFunction("greet", func(args map[string]interface{}, ctx *Context) (interface{}, error) {
		name, _ := args["name"].(string)
		if name == "" {
			name = "World"
		}
		return map[string]interface{}{
			"message": fmt.Sprintf("Hello, %s!", name),
			"time":    time.Now().Format(time.RFC3339),
		}, nil
	})

	r.RegisterFunction("add", func(args map[string]interface{}, ctx *Context) (interface{}, error) {
		a, _ := args["a"].(float64)
		b, _ := args["b"].(float64)
		return a + b, nil
	})

	r.RegisterFunction("create_user", func(args map[string]interface{}, ctx *Context) (interface{}, error) {
		email, _ := args["email"].(string)
		name, _ := args["name"].(string)

		user := map[string]interface{}{
			"id":         fmt.Sprintf("user_%d", time.Now().UnixNano()),
			"email":      email,
			"name":       name,
			"created_at": time.Now().Format(time.RFC3339),
		}

		ctx.Set("last_user", user)
		return user, nil
	})

	r.RegisterFunction("get_context", func(args map[string]interface{}, ctx *Context) (interface{}, error) {
		key, _ := args["key"].(string)
		return ctx.Get(key), nil
	})

	r.RegisterAssertion("equals", func(params map[string]interface{}, ctx *Context) AssertionResult {
		actual := params["actual"]
		expected := params["expected"]

		success := fmt.Sprintf("%v", actual) == fmt.Sprintf("%v", expected)
		var message string
		if !success {
			message = fmt.Sprintf("expected %v but got %v", expected, actual)
		}

		return AssertionResult{
			Success:  success,
			Message:  message,
			Actual:   actual,
			Expected: expected,
		}
	})

	r.RegisterAssertion("user_exists", func(params map[string]interface{}, ctx *Context) AssertionResult {
		email, _ := params["email"].(string)
		user := ctx.Get("last_user")

		if user == nil {
			return AssertionResult{
				Success: false,
				Message: "no user in context",
			}
		}

		userMap, _ := user.(map[string]interface{})
		userEmail, _ := userMap["email"].(string)

		if userEmail != email {
			return AssertionResult{
				Success:  false,
				Message:  fmt.Sprintf("user email mismatch: expected %s, got %s", email, userEmail),
				Actual:   userEmail,
				Expected: email,
			}
		}

		return AssertionResult{
			Success: true,
			Actual:  userEmail,
		}
	})

	r.RegisterHook("before_all", func(ctx *Context) error {
		fmt.Fprintln(os.Stderr, "Setting up test environment...")
		ctx.Set("test_started", time.Now().Format(time.RFC3339))
		return nil
	})

	r.RegisterHook("after_all", func(ctx *Context) error {
		fmt.Fprintln(os.Stderr, "Cleaning up test environment...")
		return nil
	})

	r.RegisterHook("before_each", func(ctx *Context) error {
		ctx.Set("step_started", time.Now().Format(time.RFC3339))
		return nil
	})

	r.RegisterHook("after_each", func(ctx *Context) error {
		return nil
	})

	return r
}

var Registry Registry = createExampleRegistry()
