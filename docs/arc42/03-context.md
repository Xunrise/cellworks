> Status: draft

# 3. Context and Scope

## 3.1 Context view

```mermaid
flowchart LR
  User([Engineer])
  CW[Cellworks]
  FS[(Project folders)]
  Git[git]
  CC[Claude Code]
  TR[(Claude transcripts<br/>~/.claude/projects)]
  NV[Neovim]
  AT[Atlassian remote MCP]
  K8s[Kubernetes API<br/>via kubectl]
  TF[(Terraform files)]
  SSH[ssh]

  User -->|keyboard, mouse| CW
  CW -->|read, watch| FS
  CW -->|status, numstat, branches, worktrees| Git
  CW -->|create, steer, observe| CC
  CW -->|tail| TR
  CW -->|embed, msgpack-rpc| NV
  CW -->|OAuth + JSON-RPC over HTTPS| AT
  CW -->|read-only get| K8s
  CW -->|read| TF
  CW -->|reachability probe| SSH
```

| Neighbour | What Cellworks does with it |
|---|---|
| Engineer | Navigates the map, edits files, creates and steers Claude Code sessions. |
| Project folders | Scans code, domain docs, Knex migrations, proto files and DTOs; watches for changes to rescan. |
| git | Reads the files a repository shows (`ls-files`), status, numstat, branches and worktrees; switches branches and adds worktrees on request. |
| Claude Code | Creates and steers sessions that survive restarts of Cellworks. |
| Claude transcripts | Discovers every session on the machine and follows what it reads and writes. |
| Neovim | Embedded editor. |
| Atlassian remote MCP | Syncs Jira projects. |
| Kubernetes API | Reads live workloads, read-only. |
| Terraform files | Reads infrastructure blocks per environment. |
| ssh | Probes once per launch whether each configured remote host is reachable. |

Out of scope: writing to clusters, any network server run by Cellworks.

## 3.2 Foreign systems

All foreign systems are local tools or files except Atlassian. "Contact for outages" is the product owner for local
tools; there is no SLA.

### Claude Code and transcripts

- **Protocol:** the `claude` CLI as a child process ([ADR-0010](../adr/0010-claude-code-background-sessions.md)). `claude --bg -n <name> "<task>"` starts a background
  session and prints its short id; `claude agents --json` lists every interactive and background session with its id,
  session id, working directory, name, status (busy, idle) and state (working, done, blocked); `claude attach <id>`
  opens a session in a pseudo-terminal; `claude stop <id>` stops it, and `claude --resume <sessionId> --bg "<prompt>"`
  continues a stopped one. Transcripts are JSON lines under `~/.claude/projects/**/*.jsonl`, tailed read-only.
- **Data:** file reads and writes from the `Read`, `Edit`, `Write`, `MultiEdit` and `NotebookEdit` tool calls in the
  transcript; pull request URLs from the result of a `gh pr create` call; session state from `claude agents --json`.
- **Auth:** the user's own Claude Code login; Cellworks never handles its credentials. `claude` runs with a clean
  environment, because a child that inherits a host's Claude environment fails to authenticate.
- **Quirks:** neither the transcript format nor `claude agents --json` is a published contract; both can change with
  Claude Code releases, so parse tolerantly and ignore unknown events and fields. Waiting for approval shows only in
  `claude agents --json`. Reads through Bash (`cat`, `grep`) are not file events. Sessions often touch files outside
  their working directory. `--resume` on a session that is still running starts a copy under a new id.

### git

- **Protocol:** the `git` CLI, run per project.
- **Data:** the files a repository shows and does not ignore (`ls-files`), status, numstat, branch and worktree lists.
- **Auth:** none beyond the user's git setup; Cellworks does not push or fetch.

### Neovim

- **Protocol:** `nvim --embed`, msgpack-RPC over stdio, `ext_linegrid` UI.
- **Data:** grid updates out, input and commands in.
- **Auth:** none.
- **Quirks:** requires a user-installed `nvim`; auto-save is configured by Cellworks so git sees changes.

### Atlassian remote MCP

- **Protocol:** JSON-RPC over HTTPS to Atlassian's remote MCP server.
- **Data:** Jira issues, components, epics, sub-tasks, links.
- **Auth:** Cellworks is its own OAuth client: dynamic client registration plus PKCE; tokens stored locally,
  never passed on a command line.
- **Availability / contact:** Atlassian status page; outages degrade to the cached tickets.
- **Quirks:** search results arrive in two shapes (`{issues, isLast, nextPageToken}` or `{issues: {nodes, pageInfo}}`);
  OAuth callback parameters are URL-encoded and decoded exactly once. Relaying through a headless Claude session was
  rejected (about 1M tokens per sync).

### Kubernetes (via kubectl)

- **Protocol:** `kubectl get … -o json` against the user's kubeconfigs.
- **Data:** workloads, services, ingresses, environment variables (`*_SERVICE_HOST` for wiring), health.
- **Auth:** the user's kubeconfig; read-only by constraint.
- **Availability / contact:** the cluster's operators; an unreachable cluster leaves the Terraform view intact.

### Terraform files

- **Protocol:** files read from an infrastructure repository (`otc:<folder>` projects).
- **Data:** resource blocks grouped per environment.
- **Auth:** none; no `terraform` command is run.

### ssh

- **Protocol:** the `ssh` CLI, `ssh -o BatchMode=yes -o ConnectTimeout=5 -- <host> true`, run in the background once per launch for each configured host.
- **Data:** the exit status only; nothing is read from the host.
- **Auth:** the user's own ssh config and keys; Cellworks never prompts for or handles credentials.
- **Quirks:** an unreachable or password-only host reads as offline; a missing `ssh` binary marks all hosts offline.
