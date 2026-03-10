# Ralph Fix Plan

## High Priority
- [x] [#91] Runtime provider abstraction for tmux and container sessions
- [x] [#92] Persist runtime metadata and service state per session
- [x] [#93] Add local Docker Compose session runtime with isolated services
- [x] [#94] Show and manage mixed tmux and container runtimes in TUI and CLI
- [x] [#95] Run session runtimes on remote Docker hosts over SSH
- [x] [#96] Hand off sessions between local and remote runtimes
- [x] [#97] Add remote host registry, placement rules, and idle session reaping
- [x] [#98] Add provider-aware runtime details and handoff actions to the TUI
- [x] [#99] Add runtime selection and remote host choice to the TUI spawn flow
- [x] [#100] Add remote host registry management to the TUI
- [x] [#101] Add idle runtime review and reap controls to the TUI


## Medium Priority


## Low Priority


## Completed
- [x] Project enabled for Ralph

## Notes
- Follow issue dependency order from top to bottom.
- Preserve tmux parity while introducing provider-neutral runtime support.
- Use `.ralph/specs/runtime-roadmap.md` for the higher-level runtime architecture.
- Do not mark the runtime work complete until the TUI parity issues (#98-#101) are implemented.
- Update this file after each major milestone.
