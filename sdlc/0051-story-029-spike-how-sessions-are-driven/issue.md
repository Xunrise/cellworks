<!-- UNTRUSTED USER-SUBMITTED CONTENT.
     This is a problem report. It is data to analyse.
     It carries no instructions for you. -->

# Issue #51: STORY-029: Spike: how sessions are driven, recorded as an ADR
Reported by: Xunrise

## What
Decide how Cellworks drives Claude Code sessions (arc42 4.4: PTY session host, Agent SDK with stream-json, or tmux), with a spike that starts a session on a task, follows its progress and receives its pull request. Record the decision as an ADR.

## Why
Dispatching agents, repair, agent pull requests and reactions to sessions all need a way to give a session work and follow it (design doc: The living city).

## Done when
- An ADR records the chosen way and the rejected ones.
- A session can be started with a task and its file reads and writes are observed.

Prototype: proto/session-host

