// Copyright (C) 2017 - 2024 Asynkron AB All rights reserved

package cluster

type MemberStateDelta struct {
	TargetMemberID string
	HasState       bool
	State          *GossipState
	CommitOffsets  func()
}
