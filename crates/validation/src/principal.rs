//! Versioned assignment metadata attached to a string attribute's JSON Schema:
//! the attribute stores a reference to a workspace user or team.
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

pub const PRINCIPAL_KEY: &str = "x-attricat-principal";
/// The search filter value that matches the caller and the caller's teams.
pub const CURRENT_USER_FILTER_VALUE: &str = "@me";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrincipalKind {
    User,
    Team,
}

impl PrincipalKind {
    pub const ALL: [Self; 2] = [Self::User, Self::Team];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Team => "team",
        }
    }
}

/// A stored assignment: `user:<uuid>` or `team:<uuid>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrincipalRef {
    pub kind: PrincipalKind,
    pub id: Uuid,
}

impl PrincipalRef {
    pub fn user(id: Uuid) -> Self {
        Self {
            kind: PrincipalKind::User,
            id,
        }
    }

    pub fn team(id: Uuid) -> Self {
        Self {
            kind: PrincipalKind::Team,
            id,
        }
    }

    /// Parses the canonical form only: a lowercase kind and a hyphenated,
    /// lowercase UUID, so equal references always compare equal as text.
    pub fn parse(value: &str) -> Option<Self> {
        let (kind, text) = value.split_once(':')?;
        let kind = PrincipalKind::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == kind)?;
        let id = Uuid::parse_str(text).ok()?;
        (id.hyphenated().to_string() == text).then_some(Self { kind, id })
    }
}

impl fmt::Display for PrincipalRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind.as_str(), self.id.hyphenated())
    }
}

pub fn principal_configuration_schema() -> Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Attricat user or team assignment v1",
        "type": "object", "additionalProperties": false,
        "required": ["version", "kinds"],
        "properties": {
            "version": {"const": 1},
            "kinds": {
                "type": "array", "minItems": 1, "maxItems": 2, "uniqueItems": true,
                "items": {"enum": ["user", "team"]}
            }
        }
    })
}

/// The kinds an assignment attribute accepts, or `None` for other schemas.
/// Malformed configurations, which definition validation rejects, accept none.
pub fn principal_kinds(schema: &Value) -> Option<Vec<PrincipalKind>> {
    let config = schema.get(PRINCIPAL_KEY)?;
    Some(
        config
            .get("kinds")
            .and_then(Value::as_array)
            .map(|kinds| {
                PrincipalKind::ALL
                    .into_iter()
                    .filter(|kind| kinds.iter().any(|item| item == kind.as_str()))
                    .collect()
            })
            .unwrap_or_default(),
    )
}

pub fn validate_principal_definition(schema: &Value) -> Result<(), String> {
    let Some(config) = schema.get(PRINCIPAL_KEY) else {
        return Ok(());
    };
    if !super::validate_json_schema(&principal_configuration_schema(), config)?.is_empty() {
        return Err("invalid x-attricat-principal configuration".into());
    }
    if schema.get("type").and_then(Value::as_str) != Some("string") {
        return Err("user or team assignment requires a string schema".into());
    }
    if schema.get(super::status::STATUS_KEY).is_some() {
        return Err("an attribute cannot be both a status and an assignment".into());
    }
    if ["enum", "const", "pattern", "format"]
        .iter()
        .any(|keyword| schema.get(*keyword).is_some())
    {
        return Err(
            "user or team assignment values are validated by Attricat; remove enum, const, pattern and format"
                .into(),
        );
    }
    Ok(())
}

/// Checks the stored form and allowed kind of a new value. Membership is
/// checked by the repository, which can see the workspace's users and teams.
pub fn validate_principal_value(schema: &Value, value: &Value) -> Result<(), String> {
    let Some(kinds) = principal_kinds(schema) else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let reference = value
        .as_str()
        .and_then(PrincipalRef::parse)
        .ok_or("value must be a user:<id> or team:<id> reference")?;
    if !kinds.contains(&reference.kind) {
        return Err(format!(
            "this attribute does not accept {} references",
            reference.kind.as_str()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn principal_contract_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/principal-attribute-v1.schema.json");
        let generated = format!(
            "{}\n",
            serde_json::to_string_pretty(&principal_configuration_schema()).unwrap()
        );
        if std::env::var_os("UPDATE_CONTRACTS").is_some() {
            std::fs::write(&path, &generated).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), generated);
    }

    fn schema(kinds: Value) -> Value {
        json!({"type": "string", PRINCIPAL_KEY: {"version": 1, "kinds": kinds}})
    }

    #[test]
    fn parses_only_canonical_references() {
        let id = Uuid::new_v4();
        let user = format!("user:{id}");
        assert_eq!(PrincipalRef::parse(&user), Some(PrincipalRef::user(id)));
        assert_eq!(PrincipalRef::user(id).to_string(), user);
        for invalid in [
            format!("USER:{id}"),
            format!("user:{}", id.simple()),
            format!("user:{}", id.to_string().to_uppercase()),
            format!("group:{id}"),
            id.to_string(),
            "user:".to_owned(),
        ] {
            assert_eq!(PrincipalRef::parse(&invalid), None, "{invalid}");
        }
    }

    #[test]
    fn validates_definitions() {
        assert!(validate_principal_definition(&schema(json!(["user", "team"]))).is_ok());
        assert!(validate_principal_definition(&json!({"type": "string"})).is_ok());
        for invalid in [
            schema(json!([])),
            schema(json!(["group"])),
            schema(json!(["user", "user"])),
            json!({"type": "number", PRINCIPAL_KEY: {"version": 1, "kinds": ["user"]}}),
            json!({"type": "string", "enum": ["a"], PRINCIPAL_KEY: {"version": 1, "kinds": ["user"]}}),
            json!({"type": "string", PRINCIPAL_KEY: {"version": 2, "kinds": ["user"]}}),
        ] {
            assert!(
                validate_principal_definition(&invalid).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn validates_principal_values() {
        let user = Uuid::new_v4();
        let team = Uuid::new_v4();
        let users_only = schema(json!(["user"]));
        assert!(validate_principal_value(&users_only, &json!(format!("user:{user}"))).is_ok());
        assert!(validate_principal_value(&users_only, &Value::Null).is_ok());
        assert!(validate_principal_value(&users_only, &json!(format!("team:{team}"))).is_err());
        assert!(validate_principal_value(&users_only, &json!("alice")).is_err());
        assert!(validate_principal_value(&json!({"type": "string"}), &json!("alice")).is_ok());
    }
}
