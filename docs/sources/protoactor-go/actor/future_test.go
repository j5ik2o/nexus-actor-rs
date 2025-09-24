package actor

import (
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestFuture_PipeTo_Message(t *testing.T) {
	a1, p1 := spawnMockProcess("a1")
	a2, p2 := spawnMockProcess("a2")
	a3, p3 := spawnMockProcess("a3")
	defer func() {
		removeMockProcess(a1)
		removeMockProcess(a2)
		removeMockProcess(a3)
	}()

	f := NewFuture(system, 1*time.Second)

	p1.On("SendUserMessage", a1, "hello")
	p2.On("SendUserMessage", a2, "hello")
	p3.On("SendUserMessage", a3, "hello")

	f.PipeTo(a1)
	f.PipeTo(a2)
	f.PipeTo(a3)

	ref, _ := system.ProcessRegistry.Get(f.pid)
	assert.IsType(t, &futureProcess{}, ref)
	fp, _ := ref.(*futureProcess)

	fp.SendUserMessage(f.pid, "hello")
	p1.AssertExpectations(t)
	p2.AssertExpectations(t)
	p3.AssertExpectations(t)
	assert.Empty(t, fp.pipes, "pipes were not cleared")
}

func TestFuture_PipeTo_TimeoutSendsError(t *testing.T) {
	a1, p1 := spawnMockProcess("a1")
	a2, p2 := spawnMockProcess("a2")
	a3, p3 := spawnMockProcess("a3")
	defer func() {
		removeMockProcess(a1)
		removeMockProcess(a2)
		removeMockProcess(a3)
	}()

	p1.On("SendUserMessage", a1, ErrTimeout)
	p2.On("SendUserMessage", a2, ErrTimeout)
	p3.On("SendUserMessage", a3, ErrTimeout)

	f := NewFuture(system, 10*time.Millisecond)
	ref, _ := system.ProcessRegistry.Get(f.pid)

	f.PipeTo(a1)
	f.PipeTo(a2)
	f.PipeTo(a3)

	err := f.Wait()
	assert.Error(t, err)

	assert.IsType(t, &futureProcess{}, ref)
	fp, _ := ref.(*futureProcess)

	p1.AssertExpectations(t)
	p2.AssertExpectations(t)
	p3.AssertExpectations(t)
	assert.Empty(t, fp.pipes, "pipes were not cleared")
}

func TestNewFuture_TimeoutNoRace(t *testing.T) {

	future := NewFuture(system, 1*time.Microsecond)
	a := rootContext.Spawn(PropsFromFunc(func(context Context) {
		switch context.Message().(type) {
		case *Started:
			context.Send(future.PID(), EchoResponse{})
		}
	}))
	_ = rootContext.StopFuture(a).Wait()
	_, _ = future.Result()
}

func assertFutureSuccess(future *Future, t *testing.T) interface{} {
	res, err := future.Result()
	assert.NoError(t, err, "timed out")
	return res
}

func TestFuture_Result_DeadLetterResponse(t *testing.T) {
	a := assert.New(t)

	future := NewFuture(system, 1*time.Second)
	rootContext.Send(future.PID(), &DeadLetterResponse{})
	resp, err := future.Result()
	a.Equal(ErrDeadLetter, err)
	a.Nil(resp)
}

func TestFuture_Result_Timeout(t *testing.T) {
	a := assert.New(t)

	future := NewFuture(system, 1*time.Second)
	resp, err := future.Result()
	a.Equal(ErrTimeout, err)
	a.Nil(resp)
}

func TestFuture_Result_Success(t *testing.T) {
	a := assert.New(t)

	future := NewFuture(system, 1*time.Second)
	rootContext.Send(future.PID(), EchoResponse{})
	resp := assertFutureSuccess(future, t)
	a.Equal(EchoResponse{}, resp)
}

func testWork(ctx Context) {
	if _, ok := ctx.Message().(string); ok {
		ctx.Respond("pong")
	}
}

func BenchmarkProto(b *testing.B) {
	system := NewActorSystem()
	pid := system.Root.Spawn(PropsFromFunc(testWork))
	for i := 0; i < b.N; i++ {
		_, err := system.Root.RequestFuture(pid, "ping", time.Second).Result()
		if err != nil {
			panic(err)
		}
	}
}
