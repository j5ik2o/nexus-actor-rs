package cluster

import (
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// Used to query the GossipActor about a given key status
type GetGossipStateRequest struct {
	GossipStateKey string
}

// Create a new GetGossipStateRequest value and return it back
func NewGetGossipStateRequest(key string) GetGossipStateRequest {
	request := GetGossipStateRequest{GossipStateKey: key}
	return request
}

// Used by the GossipActor to send back the status value of a given key
type GetGossipStateResponse struct {
	State map[string]*GossipKeyValue
}

func NewGetGossipStateResponse(state map[string]*GossipKeyValue) GetGossipStateResponse {
	value := GetGossipStateResponse{
		State: state,
	}
	return value
}

// Used to setup Gossip State Keys in the GossipActor
type SetGossipState struct {
	GossipStateKey string
	Value          proto.Message
}

// Used to set Gossip State containing GossipMap data type in the GossipActor
type SetGossipMapState struct {
	GossipStateKey string
	MapKey         string
	Value          proto.Message
}

// Used to query the Gossip State containing GossipMap data type in the GossipActor
type GetGossipMapStateRequest struct {
	GossipStateKey string
	MapKey         string
}

// Used by the GossipActor to send back the GossipMap value of a given key
type GetGossipMapStateResponse struct {
	Value *anypb.Any
}

// Used to remove Gossip State containing GossipMap data type in the GossipActor
type RemoveGossipMapState struct {
	GossipStateKey string
	MapKey         string
}

// Used to query the GossipActor about the keys in a GossipMap
type GetGossipMapKeysRequest struct {
	GossipStateKey string
}

// Used by the GossipActor to send back the keys in a GossipMap
type GetGossipMapKeysResponse struct {
	MapKeys []string
}

// Create a new SetGossipState value with the given data and return it back
func NewGossipStateKey(key string, value proto.Message) SetGossipState {
	statusKey := SetGossipState{
		GossipStateKey: key,
		Value:          value,
	}
	return statusKey
}

type SendGossipStateRequest struct{}

type SendGossipStateResponse struct{}

// Used by the GossipActor to respond SetGossipStatus requests
type SetGossipStateResponse struct{}

type AddConsensusCheck struct {
	ID    string
	Check *ConsensusCheck
}

// Mimic .NET ReenterAfterCancellation on GossipActor
type RemoveConsensusCheck struct {
	ID string
}

func NewAddConsensusCheck(id string, check *ConsensusCheck) AddConsensusCheck {
	value := AddConsensusCheck{
		ID:    id,
		Check: check,
	}
	return value
}

func NewRemoveConsensusCheck(id string) RemoveConsensusCheck {
	value := RemoveConsensusCheck{
		ID: id,
	}
	return value
}
