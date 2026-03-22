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
}

pub use inner::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_waiting_returns_vec() {
        let results = scan_waiting();
        let _ = results;
    }
}
