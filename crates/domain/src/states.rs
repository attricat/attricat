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
    /// Where one record's migration to a newer blueprint revision stands.
    pub enum RecordMigrationStatus (UnknownRecordMigrationStatus: "record migration status") {
        /// The preview found no issues; the migration can apply as is.
        Ready => "ready",
        NeedsInput => "needs_input",
        Blocked => "blocked",
        /// Reserved by a migration batch and not yet previewed.
        Pending => "pending",
        Migrating => "migrating",
        Migrated => "migrated",
        Failed => "failed",
        Skipped => "skipped",
        Superseded => "superseded",
    }
}

crate::code_enum! {
    /// Where an uploaded file stands. Only ready files are served.
    pub enum FileStatus (UnknownFileStatus: "file status") {
        Uploading => "uploading",
        /// Accepted and waiting for the file worker, which renders image
        /// variants.
        Queued => "queued",
        Processing => "processing",
        Ready => "ready",
        Failed => "failed",
        Deleted => "deleted",
    }
}

crate::code_enum! {
    /// Where a rule finding stands. A re-check that no longer finds the
    /// problem resolves it.
    pub enum RuleFindingState (UnknownRuleFindingState: "rule finding state") {
        Open => "open",
        /// Seen by someone who manages rules; it stays until resolved.
        Acknowledged => "acknowledged",
        Resolved => "resolved",
    }
}

crate::code_enum! {
    /// How prominently a rule finding is reported, least severe first.
    pub enum RuleFindingSeverity (UnknownRuleFindingSeverity: "rule finding severity") {
        Info => "info",
        Warning => "warning",
        Error => "error",
        Critical => "critical",
    }
}

crate::code_enum! {
    /// Why a data quality rule does not apply to a blueprint revision. The
    /// rule skips that revision's records: no findings, no enforcement.
    pub enum RuleInapplicability (UnknownRuleInapplicability: "rule inapplicability") {
        /// The revision has no attribute that the rule names.
        AttributeMissing => "attribute_missing",
        /// An attribute that the rule names has a type the rule cannot check.
        AttributeTypeChanged => "attribute_type_changed",
        /// A guarded status transition names an attribute that is not a
        /// status, or a status code the attribute does not have.
        StatusChanged => "status_changed",
    }
}

crate::code_enum! {
    /// Records by their unresolved rule findings. Every record falls in
    /// exactly one.
    pub enum DataQualityFilter (UnknownDataQualityFilter: "data quality filter") {
        /// At least one open finding.
        Open => "open",
        /// Unresolved findings, every one acknowledged.
        Acknowledged => "acknowledged",
        /// No unresolved findings.
        Clean => "clean",
    }
}

crate::code_enum! {
    /// Whether a workspace membership grants access. Only active memberships
    /// are authorized.
    pub enum MembershipState (UnknownMembershipState: "membership state") {
        Active => "active",
        Inactive => "inactive",
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
    /// Where a leased workflow, rule or extension operation run stands. Event
    /// deliveries use the same codes but are never cancelled.
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

crate::code_enum! {
    /// Where a file processing job stands.
    pub enum FileJobStatus (UnknownFileJobStatus: "file job status") {
        Queued => "queued",
        Running => "running",
        /// Failed an attempt and waits for its backoff before the next one.
        Retryable => "retryable",
        Completed => "completed",
        Failed => "failed",
    }
}

crate::code_enum! {
    /// What a file processing job does.
    pub enum FileJobKind (UnknownFileJobKind: "file job kind") {
        /// Records metadata and renders image variants.
        Metadata => "metadata",
        /// Extracts the text of a PDF, Word or Excel file.
        TextExtraction => "text_extraction",
        /// Deletes the objects of a file past its deletion grace.
        Purge => "purge",
    }
}

crate::code_enum! {
    /// Whether a file's text was extracted.
    pub enum FileTextExtractionStatus (UnknownFileTextExtractionStatus: "file text extraction status") {
        Ready => "ready",
        Failed => "failed",
    }
}

crate::code_enum! {
    /// Why a file has no extracted text.
    pub enum TextExtractionFailure (UnknownTextExtractionFailure: "text extraction failure") {
        Encrypted => "encrypted",
        /// A PDF of scanned pages without a text layer.
        NoTextLayer => "no_text_layer",
        TooLarge => "too_large",
        Malformed => "malformed",
        Unsupported => "unsupported",
        TimedOut => "timed_out",
    }
}

crate::code_enum! {
    /// Whether an extension connection can issue access tokens.
    pub enum ConnectionStatus (UnknownConnectionStatus: "connection status") {
        PendingAuthorization => "pending_authorization",
        Active => "active",
        Failed => "failed",
        Revoked => "revoked",
    }
}

crate::code_enum! {
    /// Where an extension operation's artifact upload stands.
    pub enum ArtifactState (UnknownArtifactState: "artifact state") {
        Incomplete => "incomplete",
        Completed => "completed",
        Aborted => "aborted",
    }
}

crate::code_enum! {
    /// The outcome of one target record of a workflow action that updates
    /// referencing records.
    pub enum WorkflowTargetStatus (UnknownWorkflowTargetStatus: "workflow target status") {
        Completed => "completed",
        /// Retried while the run retries.
        Failed => "failed",
        /// No longer referenced the trigger record when revisited.
        Skipped => "skipped",
    }
}

crate::code_enum! {
    /// The outcome of one extension event handler for one event.
    pub enum HandlerAttemptStatus (UnknownHandlerAttemptStatus: "handler attempt status") {
        Completed => "completed",
        /// Failed and due again at its next attempt time.
        Retrying => "retrying",
        /// Waiting out the handler's debounce window.
        Debouncing => "debouncing",
        /// A newer event for the same record replaced it.
        Superseded => "superseded",
        Rejected => "rejected",
        /// Out of attempts.
        Exhausted => "exhausted",
    }
}

impl HandlerAttemptStatus {
    /// Whether the handler is done with the event.
    pub const fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Superseded | Self::Rejected | Self::Exhausted
        )
    }
}

crate::code_enum! {
    /// Where an extension operation's brokered HTTP delivery stands.
    pub enum HttpDeliveryState (UnknownHttpDeliveryState: "HTTP delivery state") {
        /// Sent without a recorded response, so it may or may not have applied.
        Uncertain => "uncertain",
        Succeeded => "succeeded",
        Failed => "failed",
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
            HttpDeliveryState::ALL,
            HttpDeliveryState::as_str,
            HttpDeliveryState::parse,
        );
        crate::codes::assert_codes_round_trip(
            HandlerAttemptStatus::ALL,
            HandlerAttemptStatus::as_str,
            HandlerAttemptStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            FileJobStatus::ALL,
            FileJobStatus::as_str,
            FileJobStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            FileJobKind::ALL,
            FileJobKind::as_str,
            FileJobKind::parse,
        );
        crate::codes::assert_codes_round_trip(
            FileTextExtractionStatus::ALL,
            FileTextExtractionStatus::as_str,
            FileTextExtractionStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            TextExtractionFailure::ALL,
            TextExtractionFailure::as_str,
            TextExtractionFailure::parse,
        );
        crate::codes::assert_codes_round_trip(
            ConnectionStatus::ALL,
            ConnectionStatus::as_str,
            ConnectionStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            ArtifactState::ALL,
            ArtifactState::as_str,
            ArtifactState::parse,
        );
        crate::codes::assert_codes_round_trip(
            WorkflowTargetStatus::ALL,
            WorkflowTargetStatus::as_str,
            WorkflowTargetStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            RecordMigrationStatus::ALL,
            RecordMigrationStatus::as_str,
            RecordMigrationStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            FileStatus::ALL,
            FileStatus::as_str,
            FileStatus::parse,
        );
        crate::codes::assert_codes_round_trip(
            RuleFindingState::ALL,
            RuleFindingState::as_str,
            RuleFindingState::parse,
        );
        crate::codes::assert_codes_round_trip(
            RuleFindingSeverity::ALL,
            RuleFindingSeverity::as_str,
            RuleFindingSeverity::parse,
        );
        crate::codes::assert_codes_round_trip(
            RuleInapplicability::ALL,
            RuleInapplicability::as_str,
            RuleInapplicability::parse,
        );
        crate::codes::assert_codes_round_trip(
            DataQualityFilter::ALL,
            DataQualityFilter::as_str,
            DataQualityFilter::parse,
        );
        crate::codes::assert_codes_round_trip(
            MembershipState::ALL,
            MembershipState::as_str,
            MembershipState::parse,
        );
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
