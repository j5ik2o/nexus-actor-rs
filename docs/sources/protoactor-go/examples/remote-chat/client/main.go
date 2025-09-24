package main

import (
	"log"

	"chat/messages"

	console "github.com/asynkron/goconsole"
	"github.com/asynkron/protoactor-go/actor"
	"github.com/asynkron/protoactor-go/remote"
)

func main() {
	system := actor.NewActorSystem()
	config := remote.Configure("127.0.0.1", 0)
	remoter := remote.NewRemote(system, config)
	remoter.Start()

	server := actor.NewPID("127.0.0.1:8080", "chatserver")

	// define root context
	rootContext := system.Root

	// spawn our chat client inline
	props := actor.PropsFromFunc(func(context actor.Context) {
		switch msg := context.Message().(type) {
		case *messages.Connected:
			log.Println(msg.Message)
		case *messages.SayResponse:
			log.Printf("%v: %v", msg.UserName, msg.Message)
		case *messages.NickResponse:
			log.Printf("%v is now known as %v", msg.OldUserName, msg.NewUserName)
		}
	})

	client := rootContext.Spawn(props)

	rootContext.Send(server, &messages.Connect{
		Sender: client,
	})

	nick := "Roger"
	cons := console.NewConsole(func(text string) {
		rootContext.Send(server, &messages.SayRequest{
			UserName: nick,
			Message:  text,
		})
	})
	// write /nick NAME to change your chat username
	cons.Command("/nick", func(newNick string) {
		rootContext.Send(server, &messages.NickRequest{
			OldUserName: nick,
			NewUserName: newNick,
		})
		nick = newNick
	})
	cons.Run()
}
