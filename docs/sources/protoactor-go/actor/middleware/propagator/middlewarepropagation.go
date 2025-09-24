package propagator

import (
	"github.com/asynkron/protoactor-go/actor"
)

type MiddlewarePropagator struct {
	spawnMiddleware    []actor.SpawnMiddleware
	senderMiddleware   []actor.SenderMiddleware
	receiverMiddleware []actor.ReceiverMiddleware
	contextDecorators  []actor.ContextDecorator
}

func New() *MiddlewarePropagator {
	return &MiddlewarePropagator{}
}

func (propagator *MiddlewarePropagator) WithItselfForwarded() *MiddlewarePropagator {
	return propagator.WithSpawnMiddleware(propagator.SpawnMiddleware)
}

func (propagator *MiddlewarePropagator) WithSpawnMiddleware(middleware ...actor.SpawnMiddleware) *MiddlewarePropagator {
	propagator.spawnMiddleware = append(propagator.spawnMiddleware, middleware...)
	return propagator
}

func (propagator *MiddlewarePropagator) WithSenderMiddleware(middleware ...actor.SenderMiddleware) *MiddlewarePropagator {
	propagator.senderMiddleware = append(propagator.senderMiddleware, middleware...)
	return propagator
}

func (propagator *MiddlewarePropagator) WithReceiverMiddleware(middleware ...actor.ReceiverMiddleware) *MiddlewarePropagator {
	propagator.receiverMiddleware = append(propagator.receiverMiddleware, middleware...)
	return propagator
}

func (propagator *MiddlewarePropagator) WithContextDecorator(decorators ...actor.ContextDecorator) *MiddlewarePropagator {
	propagator.contextDecorators = append(propagator.contextDecorators, decorators...)
	return propagator
}

func (propagator *MiddlewarePropagator) SpawnMiddleware(next actor.SpawnFunc) actor.SpawnFunc {
	return func(actorSystem *actor.ActorSystem, id string, props *actor.Props, parentContext actor.SpawnerContext) (pid *actor.PID, e error) {
		if propagator.spawnMiddleware != nil {
			props = props.Configure(actor.WithSpawnMiddleware(propagator.spawnMiddleware...))
		}
		if propagator.senderMiddleware != nil {
			props = props.Configure(actor.WithSenderMiddleware(propagator.senderMiddleware...))
		}
		if propagator.receiverMiddleware != nil {
			props = props.Configure(actor.WithReceiverMiddleware(propagator.receiverMiddleware...))
		}
		if propagator.contextDecorators != nil {
			props = props.Configure(actor.WithContextDecorator(propagator.contextDecorators...))
		}
		pid, err := next(actorSystem, id, props, parentContext)
		return pid, err
	}
}
