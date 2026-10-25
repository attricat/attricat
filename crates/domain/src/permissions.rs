//! Permission codes: what a role grants and a personal API token may carry.
//! Roles store them by code, so [`Permission::as_str`] is the stored form.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

macro_rules! permissions {
    ($($variant:ident => $code:literal),+ $(,)?) => {
        /// A workspace permission.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Permission {
            $($variant),+
        }

        impl Permission {
            /// Every permission, in declaration order.
            pub const ALL: &'static [Permission] = &[$(Self::$variant),+];

            /// The stored permission code, such as `records.read`.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }

            pub fn parse(code: &str) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

permissions! {
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
    DataHealthRead => "data_health.read",
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown permission '{0}'")]
pub struct UnknownPermission(pub String);

impl TryFrom<String> for Permission {
    type Error = UnknownPermission;

    fn try_from(code: String) -> Result<Self, Self::Error> {
        Self::parse(&code).ok_or(UnknownPermission(code))
    }
}

impl std::str::FromStr for Permission {
    type Err = UnknownPermission;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        Self::parse(code).ok_or_else(|| UnknownPermission(code.to_owned()))
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for Permission {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Self::parse(&code).ok_or_else(|| serde::de::Error::custom(UnknownPermission(code)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        let mut codes = std::collections::HashSet::new();
        for permission in Permission::ALL {
            assert!(codes.insert(permission.as_str()));
            assert_eq!(Permission::parse(permission.as_str()), Some(*permission));
        }
        assert_eq!(Permission::parse("records.reed"), None);
    }
}
