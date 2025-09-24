package actor_test

import (
	"fmt"
	"sync"
	"time"

	"github.com/asynkron/protoactor-go/actor"
)

// Demonstrates how to create an actor using a function literal and how to send a message asynchronously
func Example() {
	context := system.Root
	props := actor.PropsFromFunc(func(c actor.Context) {
		if msg, ok := c.Message().(string); ok {
			fmt.Println(msg) // outputs "Hello World"
		}
	})

	pid := context.Spawn(props)

	context.Send(pid, "Hello World")
	time.Sleep(time.Millisecond * 100)

	_ = context.StopFuture(pid).Wait() // wait for the actor to stop

	// Output: Hello World
}

// Demonstrates how to send a message from one actor to another and for the caller to wait for a response before
// proceeding
func Example_synchronous() {
	var wg sync.WaitGroup

	wg.Add(1)

	// callee will wait for the PING message
	callee := system.Root.Spawn(actor.PropsFromFunc(func(c actor.Context) {
		if msg, ok := c.Message().(string); ok {
			fmt.Println(msg) // outputs PING
			c.Respond("PONG")
		}
	}))

	// caller will send a PING message and wait for the PONG
	caller := system.Root.Spawn(actor.PropsFromFunc(func(c actor.Context) {
		switch msg := c.Message().(type) {
		// the first message an actor receives after it has started
		case *actor.Started:
			// send a PING to the callee, and specify the response
			// is sent to Self, which is this actor'pids PID
			c.Request(callee, "PING")

		case string:
			fmt.Println(msg) // PONG
			wg.Done()
		}
	}))

	wg.Wait()
	_ = system.Root.StopFuture(callee).Wait()
	_ = system.Root.StopFuture(caller).Wait()

	// Output:
	// PING
	// PONG
}
