//! Permission codes: what a role grants and a personal API token may carry.
//! Roles store them by code, so [`Permission::as_str`] is the stored form.

crate::code_enum! {
    /// A workspace permission.
    pub enum Permission (UnknownPermission: "permission") {
        RecordsRead => "records.read",
        RecordsWrite => "records.write",
        RecordsDelete => "records.delete",
        RecordsPublish => "records.publish",
        RecordsUnlock => "records.unlock",
        BlueprintsRead => "blueprints.read",
        BlueprintsWrite => "blueprints.write",
        BlueprintsPublish => "blueprints.publish",
        ContextsRead => "contexts.read",
        ContextsWrite => "contexts.write",
        ExtensionsRead => "extensions.read",
        ExtensionsManage => "extensions.manage",
        WorkspaceManage => "workspace.manage",
        MembersManage => "members.manage",
        RolesManage => "roles.manage",
        RolesGrant => "roles.grant",
        TokensManage => "tokens.manage",
        RulesRead => "rules.read",
        RulesManage => "rules.manage",
        WorkflowsRead => "workflows.read",
        WorkflowsManage => "workflows.manage",
        AgentsRun => "agents.run",
        AgentsManage => "agents.manage",
        AuditRead => "audit.read",
        FilesHold => "files.hold",
        SolutionPacksManage => "solution_packs.manage",
        WorkspaceNavigationManage => "workspace_navigation.manage",
        TagColorsManage => "tag_colors.manage",
        DataHealthRead => "data_health.read",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        crate::codes::assert_codes_round_trip(
            Permission::ALL,
            Permission::as_str,
            Permission::parse,
        );
        assert_eq!(Permission::parse("records.reed"), None);
    }
}
