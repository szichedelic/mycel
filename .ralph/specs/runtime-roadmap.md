# Runtime Roadmap

## Objective

Extend Mycel from a tmux-only session manager into a provider-driven runtime manager that supports:

- local tmux sessions
- isolated local container sessions
- remote container sessions on Docker-capable hosts
- session handoff from local to remote runtimes
- small remote host pools with basic placement and idle cleanup

## Design Constraints

- Preserve the current tmux workflow and user experience while adding new providers.
- Keep the TUI project-centric. Sessions remain the primary object.
- Avoid Docker-specific logic leaking through unrelated modules.
- Prefer additive database migrations.
- Keep the first remote implementation simple: SSH and Docker context before any daemon.

## Planned Execution Order

1. Runtime provider abstraction
2. Runtime and service persistence
3. Local Docker Compose session runtime
4. Provider-aware TUI and CLI session management
5. Remote Docker host sessions
6. Session handoff between local and remote runtimes
7. Host registry, placement, and idle reaping
8. TUI runtime details and handoff actions
9. TUI spawn flow runtime and host selection
10. TUI host registry management
11. TUI idle runtime review and reap controls

## Session Model

A session should be treated as:

- repo or worktree state
- runtime provider metadata
- runtime instance identity
- service topology and health
- user note and backend choice
- resume or handoff context

## Runtime Model

The runtime layer should provide a common interface for:

- inspect
- attach
- preview
- stop
- kill
- restart

The tmux-backed runtime should use the same interface as future Docker-based providers.

## Container Model

For local container sessions:

- use one Compose project per Mycel session
- namespace networks, volumes, and container names by session
- keep internal service ports private by default
- expose only user-facing ports as needed
- store app URLs and service health in session runtime metadata

Avoid Docker-in-Docker by default. Prefer host Docker with per-session Compose projects.

## Remote Model

For the first remote implementation:

- treat the remote host as a Docker-capable execution target
- connect via SSH-backed Docker context or equivalent
- reuse the same runtime and service model as local Docker sessions
- keep cloud provisioning out of scope

## Handoff Model

Treat handoff as checkpoint and rehydrate, not live process migration.

The handoff should preserve:

- repo state
- runtime configuration
- environment references
- session note and summary
- source and destination runtime metadata

## Related Issues

- #91 Runtime provider abstraction for tmux and container sessions
- #92 Persist runtime metadata and service state per session
- #93 Add local Docker Compose session runtime with isolated services
- #94 Show and manage mixed tmux and container runtimes in TUI and CLI
- #95 Run session runtimes on remote Docker hosts over SSH
- #96 Hand off sessions between local and remote runtimes
- #97 Add remote host registry, placement rules, and idle session reaping
- #98 Add provider-aware runtime details and handoff actions to the TUI
- #99 Add runtime selection and remote host choice to the TUI spawn flow
- #100 Add remote host registry management to the TUI
- #101 Add idle runtime review and reap controls to the TUI
