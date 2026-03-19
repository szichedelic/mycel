/// Agent attention detection via poke.
///
/// Wraps the poke crate to scan for AI coding agents that are
/// waiting for human input across tmux sessions.

#[cfg(feature = "poke")]
mod inner {
    use poke::models::AgentStatus;

    /// Scan for agents currently waiting for human attention.
    /// Returns an empty vec on any error (poke not configured, tmux not running, etc).
    pub fn scan_waiting() -> Vec<AgentStatus> {
        let aggregator = poke::build_aggregator();
        aggregator.scan_waiting()
    }

    /// Return the number of agents currently waiting for attention.
    pub fn waiting_count() -> usize {
        scan_waiting().len()
    }

    /// Check whether any agent is currently waiting.
    pub fn any_waiting() -> bool {
        waiting_count() > 0
    }

    /// Re-export the status type for consumers.
    pub use poke::models::{AgentStatus as Status, AgentStatusKind, WaitingType};
}

#[cfg(not(feature = "poke"))]
mod inner {
    /// Stub AgentStatus when poke feature is disabled.
    #[derive(Debug, Clone)]
    pub struct Status {
        pub agent: String,
        pub status: AgentStatusKind,
        pub waiting_type: Option<WaitingType>,
        pub summary: Option<String>,
        pub tmux_session: String,
        pub tmux_pane: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum AgentStatusKind {
        Waiting,
        Working,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum WaitingType {
        Question,
        Approval,
        Choice,
    }

    pub fn scan_waiting() -> Vec<Status> {
        Vec::new()
    }

    pub fn waiting_count() -> usize {
        0
    }

    pub fn any_waiting() -> bool {
        false
    }
}

pub use inner::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_waiting_returns_vec() {
        // Should not panic even if poke/tmux isn't available
        let results = scan_waiting();
        // We can't assert specific results since it depends on tmux state,
        // but it should return without error
        let _ = results; // just verify it doesn't panic
    }

    #[test]
    fn waiting_count_matches_scan() {
        let count = waiting_count();
        let results = scan_waiting();
        assert_eq!(count, results.len());
    }

    #[test]
    fn any_waiting_consistent() {
        let any = any_waiting();
        let count = waiting_count();
        assert_eq!(any, count > 0);
    }
}
