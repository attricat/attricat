//! Stored lifecycle states. Each enum lists the values its table's `CHECK`
//! constraint allows.

crate::code_enum! {
    /// Whether an installed extension runs in its workspace.
    pub enum ExtensionState (UnknownExtensionState: "extension state") {
        Disabled => "disabled",
        Enabled => "enabled",
        /// Stopped after a failure. Re-enabling revalidates the installed
        /// release and its configuration.
        Quarantined => "quarantined",
    }
}

crate::code_enum! {
    /// Where applying a solution-pack plan stands.
    pub enum SolutionPackApplicationState (UnknownSolutionPackApplicationState: "solution pack application state") {
        Running => "running",
        Completed => "completed",
        Failed => "failed",
        /// Permanently invalidated; it cannot resume.
        Invalid => "invalid",
        /// A resumable sample-data application stopped by an operator or at
        /// its resumability deadline.
        Abandoned => "abandoned",
    }
}

crate::code_enum! {
    /// Where one step of a solution-pack application stands.
    pub enum SolutionPackStepState (UnknownSolutionPackStepState: "solution pack step state") {
        Pending => "pending",
        Completed => "completed",
        Failed => "failed",
    }
}

crate::code_enum! {
    /// Where an agent run stands.
    pub enum AgentRunStatus (UnknownAgentRunStatus: "agent run status") {
        Queued => "queued",
        Running => "running",
        /// Paused until a person decides its pending tool calls.
        AwaitingApproval => "awaiting_approval",
        Completed => "completed",
        Failed => "failed",
        Cancelled => "cancelled",
        Skipped => "skipped",
    }
}

impl AgentRunStatus {
    /// Whether the run has finished and its status will not change.
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

crate::code_enum! {
    /// Where one tool call of an agent run stands.
    pub enum AgentToolCallState (UnknownAgentToolCallState: "agent tool call state") {
        PendingApproval => "pending_approval",
        Approved => "approved",
        Rejected => "rejected",
        Completed => "completed",
        Failed => "failed",
    }
}

crate::code_enum! {
    /// Where a record's export to one destination stands, in the order the
    /// UI lists them. Saved Explorer searches filter by them too.
    pub enum ExportStatus (UnknownExportStatus: "export status") {
        Pending => "pending",
        Retrying => "retrying",
        Uncertain => "uncertain",
        Rejected => "rejected",
        Synced => "synced",
        /// The record was removed from the destination. A deleted record
        /// accepts only this commit.
        Withdrawn => "withdrawn",
    }
}

crate::code_enum! {
    /// Where a leased workflow, rule or extension operation run stands.
    pub enum RunStatus (UnknownRunStatus: "run status") {
        Pending => "pending",
        Leased => "leased",
        Completed => "completed",
        DeadLetter => "dead_letter",
        Cancelled => "cancelled",
    }
}

impl RunStatus {
    /// Whether the run will not be leased again.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::DeadLetter | Self::Cancelled)
    }
}

crate::code_enum! {
    /// Whether a blueprint, reusable attribute, workflow or rule revision is
    /// still editable or published.
    pub enum RevisionStatus (UnknownRevisionStatus: "revision status") {
        Draft => "draft",
        Published => "published",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        crate::codes::assert_codes_round_trip(
            ExtensionState::ALL,
            ExtensionState::as_str,
            ExtensionState::parse,
        );
        crate::codes::assert_codes_round_trip(
            SolutionPackApplicationState::ALL,
            SolutionPackApplicationState::as_str,
            SolutionPackApplicationState::parse,
        );
        crate::codes::assert_codes_round_trip(
            SolutionPackStepState::ALL,
            SolutionPackStepState::as_str,
            SolutionPackStepState::parse,
        );
        crate::codes::assert_codes_round_trip(
            RevisionStatus::ALL,
            RevisionStatus::as_str,
            RevisionStatus::parse,
        );
        crate::codes::assert_codes_round_trip(RunStatus::ALL, RunStatus::as_str, RunStatus::parse);
        crate::codes::assert_codes_round_trip(
            AgentRunStatus::ALL,
            AgentRunStatus::as_str,
            AgentRunStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            AgentToolCallState::ALL,
            AgentToolCallState::as_str,
            AgentToolCallState::parse,
        );
        crate::codes::assert_codes_round_trip(
            ExportStatus::ALL,
            ExportStatus::as_str,
            ExportStatus::parse,
        );
    }
}
