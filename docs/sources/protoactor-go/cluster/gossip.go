// Copyright (C) 2017 - 2024 Asynkton AB All rights reserved

package cluster

import (
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// customary type that defines a states sender callback.
type LocalStateSender func(memberStateDelta *MemberStateDelta, member *Member)

// This interface must be implemented by any value that.
// wants to be used as a gossip state storage
type GossipStateStorer interface {
	GetState(key string) map[string]*GossipKeyValue
	SetState(key string, value proto.Message)
	SetMapState(stateKey string, mapKey string, value proto.Message)
	RemoveMapState(stateKey string, mapKey string)
	GetMapKeys(stateKey string) []string
	GetMapState(stateKey string, mapKey string) *anypb.Any
}

// This interface must be implemented by any value that
// wants to add or remove consensus checkers
type GossipConsensusChecker interface {
	AddConsensusCheck(id string, check *ConsensusCheck)
	RemoveConsensusCheck(id string)
}

// This interface must be implemented by any value that
// wants to react to cluster topology events
type GossipCore interface {
	UpdateClusterTopology(topology *ClusterTopology)
	ReceiveState(remoteState *GossipState) []*GossipUpdate
	SendState(sendStateToMember LocalStateSender)
	GetMemberStateDelta(targetMemberID string) *MemberStateDelta
}

// The Gossip interface must be implemented by any value
// that pretends to participate with-in the Gossip protocol
type Gossip interface {
	GossipStateStorer
	GossipConsensusChecker
	GossipCore
}
