# Ralph Fix Plan

## High Priority
- [x] [#102] Add poke as a dependency for agent attention detection
- [x] [#103] Show agent attention indicators in session list
- [x] [#105] Add agent attention count to TUI status bar
- [x] [#104] Add keybinding to switch to waiting agent from TUI

## Medium Priority


## Low Priority


## Completed
- [x] [#91-#101] Runtime provider abstraction, Docker Compose, remote hosts, TUI integration
- [x] Project enabled for Ralph

## Notes
- #102 must be done first — adds poke crate as a dependency
- #103 and #105 depend on #102, can be done in either order
- #104 depends on #103 (needs the indicators to know which sessions have waiting agents)
- poke repo: https://github.com/szichedelic/poke (use as git dependency)
- Handle gracefully if poke is not installed or returns errors
- Commit conventions: `{scope}: {description}` — single line, no co-authorship
