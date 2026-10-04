//! README, release notes, setup checklist, declarative checks and extension
//! configuration templates.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;

use super::{
    MAX_IDENTIFIER_BYTES, MAX_NAME_BYTES, MAX_SOLUTION_PACK_CHECKLIST_BYTES,
    MAX_SOLUTION_PACK_CHECKLIST_ITEMS, MAX_SOLUTION_PACK_CHECKS, MAX_SOLUTION_PACK_CHECKS_BYTES,
    MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_BYTES, MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_DEPTH,
    MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS,
    MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES,
    MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES, MAX_SOLUTION_PACK_README_BYTES,
    MAX_SOLUTION_PACK_RELEASE_NOTES_BYTES, SOLUTION_PACK_RESOURCE_FORMAT_VERSION,
    SolutionPackCheckDefinition, SolutionPackCheckPredicate, SolutionPackChecksFile,
    SolutionPackError, SolutionPackExtensionLayout, SolutionPackGuidance, SolutionPackManifest,
    SolutionPackSetupChecklist, invalid, is_valid_stable_code, validate_bounded_text,
};
use crate::extensions::valid_contribution_key;

pub(super) fn validate_guidance_and_checks(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
    extension_layout: &Option<SolutionPackExtensionLayout>,
) -> Result<(SolutionPackGuidance, Vec<SolutionPackCheckDefinition>), SolutionPackError> {
    let mut guidance = SolutionPackGuidance::default();
    if let Some(documentation) = &manifest.documentation {
        if let Some(reference) = &documentation.readme {
            guidance.readme_markdown = Some(validate_markdown_file(
                &files[&reference.path],
                MAX_SOLUTION_PACK_README_BYTES,
                "README",
            )?);
        }
        if let Some(reference) = &documentation.release_notes {
            guidance.release_notes_markdown = Some(validate_markdown_file(
                &files[&reference.path],
                MAX_SOLUTION_PACK_RELEASE_NOTES_BYTES,
                "release notes",
            )?);
        }
        if let Some(reference) = &documentation.setup_checklist {
            let bytes = &files[&reference.path];
            if bytes.len() > MAX_SOLUTION_PACK_CHECKLIST_BYTES {
                return invalid("solution-pack setup checklist exceeds the size limit");
            }
            let mut checklist: SolutionPackSetupChecklist =
                serde_json::from_slice(bytes).map_err(|_| {
                    SolutionPackError::Invalid(
                        "solution-pack setup checklist is not valid strict JSON".into(),
                    )
                })?;
            if checklist.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION
                || checklist.items.is_empty()
                || checklist.items.len() > MAX_SOLUTION_PACK_CHECKLIST_ITEMS
            {
                return invalid("solution-pack setup checklist format or item count is invalid");
            }
            let mut keys = HashSet::new();
            for item in &mut checklist.items {
                validate_guidance_key(&item.key, "checklist/")?;
                if !keys.insert(item.key.as_str()) {
                    return invalid(format!("duplicate setup checklist key '{}'", item.key));
                }
                validate_bounded_text(&item.title, "setup checklist title", MAX_NAME_BYTES)?;
                item.markdown =
                    validate_markdown(&item.markdown, 4096, "setup checklist markdown")?;
                if let Some(check) = &item.check {
                    validate_guidance_key(check, "checks/")?;
                }
            }
            if serde_json::to_vec(&checklist)
                .map_err(|_| {
                    SolutionPackError::Invalid(
                        "solution-pack setup checklist could not be normalized".into(),
                    )
                })?
                .len()
                > MAX_SOLUTION_PACK_CHECKLIST_BYTES
            {
                return invalid("normalized solution-pack setup checklist exceeds the size limit");
            }
            guidance.setup_checklist = Some(checklist);
        }
    }

    let checks = if let Some(reference) = &manifest.checks {
        let bytes = &files[&reference.path];
        if bytes.len() > MAX_SOLUTION_PACK_CHECKS_BYTES {
            return invalid("solution-pack checks file exceeds the size limit");
        }
        let checks_file: SolutionPackChecksFile = serde_json::from_slice(bytes).map_err(|_| {
            SolutionPackError::Invalid("solution-pack checks file is not valid strict JSON".into())
        })?;
        if checks_file.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION
            || checks_file.checks.is_empty()
            || checks_file.checks.len() > MAX_SOLUTION_PACK_CHECKS
        {
            return invalid("solution-pack checks format or check count is invalid");
        }
        let blueprint_keys = manifest
            .resources
            .blueprints
            .iter()
            .map(|resource| resource.key.as_str())
            .collect::<HashSet<_>>();
        let extensions = manifest
            .extensions
            .iter()
            .map(|requirement| (requirement.key.as_str(), requirement))
            .collect::<HashMap<_, _>>();
        let mut keys = HashSet::new();
        for check in &checks_file.checks {
            validate_guidance_key(&check.key, "checks/")?;
            if !keys.insert(check.key.as_str()) {
                return invalid(format!("duplicate check key '{}'", check.key));
            }
            validate_bounded_text(&check.title, "check title", MAX_NAME_BYTES)?;
            match &check.predicate {
                SolutionPackCheckPredicate::BlueprintPublished { blueprint }
                | SolutionPackCheckPredicate::ExploreNavigationEntryPresent { blueprint } => {
                    if !blueprint_keys.contains(blueprint.as_str()) {
                        return invalid(format!(
                            "check '{}' references undeclared blueprint '{}'",
                            check.key, blueprint
                        ));
                    }
                }
                SolutionPackCheckPredicate::ExtensionInstalled { extension }
                | SolutionPackCheckPredicate::ExtensionEnabled { extension } => {
                    if !extensions.contains_key(extension.as_str()) {
                        return invalid(format!(
                            "check '{}' references undeclared extension '{}'",
                            check.key, extension
                        ));
                    }
                }
                SolutionPackCheckPredicate::ExtensionConfigurationMatches { extension } => {
                    if extensions
                        .get(extension.as_str())
                        .is_none_or(|requirement| requirement.configuration_template.is_none())
                    {
                        return invalid(format!(
                            "check '{}' requires an extension configuration template",
                            check.key
                        ));
                    }
                }
                SolutionPackCheckPredicate::WorkspaceExtensionLayoutPlacementPresent {
                    contribution,
                } => {
                    if !valid_contribution_key(contribution)
                        || extension_layout.as_ref().map_or(0, |layout| {
                            layout
                                .entries
                                .iter()
                                .filter(|entry| entry.contribution == *contribution)
                                .count()
                        }) != 1
                    {
                        return invalid(format!(
                            "check '{}' references no unique workspace extension-layout contribution",
                            check.key
                        ));
                    }
                }
            }
        }
        checks_file.checks
    } else {
        Vec::new()
    };

    let check_keys = checks
        .iter()
        .map(|check| check.key.as_str())
        .collect::<HashSet<_>>();
    if let Some(checklist) = &guidance.setup_checklist {
        for item in &checklist.items {
            if let Some(check) = &item.check
                && !check_keys.contains(check.as_str())
            {
                return invalid(format!(
                    "setup checklist item '{}' references undeclared check '{}'",
                    item.key, check
                ));
            }
        }
    }
    Ok((guidance, checks))
}

fn validate_guidance_key(value: &str, prefix: &str) -> Result<(), SolutionPackError> {
    let suffix = value.strip_prefix(prefix).unwrap_or_default();
    if value.len() > MAX_IDENTIFIER_BYTES
        || suffix.is_empty()
        || suffix.contains('/')
        || !is_valid_stable_code(suffix)
    {
        return invalid(format!("guidance key '{value}' is invalid"));
    }
    Ok(())
}

fn validate_markdown_file(
    bytes: &[u8],
    limit: usize,
    label: &str,
) -> Result<String, SolutionPackError> {
    if bytes.len() > limit {
        return invalid(format!("solution-pack {label} exceeds the size limit"));
    }
    let markdown = std::str::from_utf8(bytes)
        .map_err(|_| SolutionPackError::Invalid(format!("solution-pack {label} must be UTF-8")))?;
    validate_markdown(markdown, limit, label)
}

pub(super) fn validate_markdown(
    markdown: &str,
    limit: usize,
    label: &str,
) -> Result<String, SolutionPackError> {
    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() || normalized.len() > limit || normalized.trim() != normalized {
        return invalid(format!(
            "solution-pack {label} must be non-empty, trimmed, bounded Markdown"
        ));
    }
    if normalized.chars().any(|character| {
        (character.is_control() && !matches!(character, '\n' | '\t'))
            || ('\u{7f}'..='\u{9f}').contains(&character)
    }) {
        return invalid(format!("solution-pack {label} contains control characters"));
    }
    // Guidance is deliberately display-only. Reject the Markdown constructs that
    // can embed active content, fetch resources, or carry executable snippets.
    let lower = normalized.to_ascii_lowercase();
    if normalized.contains('<')
        || normalized.contains('>')
        || normalized.contains("![")
        || normalized.contains('`')
        || lower.contains("javascript:")
        || lower.contains("data:")
        || lower.contains("file:")
        || lower.contains("mailto:")
    {
        return invalid(format!("solution-pack {label} contains unsafe Markdown"));
    }
    for (_, destination) in markdown_destinations(&normalized) {
        if !destination.starts_with('#') || destination.len() == 1 {
            return invalid(format!(
                "solution-pack {label} links may only target same-document fragments"
            ));
        }
    }
    for line in normalized.lines() {
        let trimmed = line.trim_start();
        if (line.starts_with("    ") || line.starts_with('\t')) && !trimmed.is_empty()
            || trimmed.starts_with("~~~")
        {
            return invalid(format!("solution-pack {label} cannot contain code blocks"));
        }
        if let Some((_, destination)) = trimmed.split_once("]:")
            && !destination.trim().starts_with('#')
        {
            return invalid(format!(
                "solution-pack {label} links may only target same-document fragments"
            ));
        }
    }
    Ok(normalized)
}

fn markdown_destinations(markdown: &str) -> Vec<(usize, &str)> {
    let mut destinations = Vec::new();
    let bytes = markdown.as_bytes();
    let mut index = 0;
    while index + 2 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b'(' {
            let start = index + 2;
            if let Some(relative_end) = markdown[start..].find(')') {
                let end = start + relative_end;
                destinations.push((start, markdown[start..end].trim()));
                index = end;
            }
        }
        index += 1;
    }
    destinations
}

pub(super) fn validate_configuration_templates(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, Value>, SolutionPackError> {
    let mut templates = BTreeMap::new();
    for requirement in &manifest.extensions {
        let Some(reference) = &requirement.configuration_template else {
            continue;
        };
        let bytes = &files[&reference.path];
        if bytes.len() > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_BYTES {
            return invalid(format!(
                "extension requirement '{}' configuration template exceeds the size limit",
                requirement.key
            ));
        }
        let template: Value = serde_json::from_slice(bytes).map_err(|_| {
            SolutionPackError::Invalid(format!(
                "extension requirement '{}' configuration template must be valid JSON",
                requirement.key
            ))
        })?;
        if !template.is_object() {
            return invalid(format!(
                "extension requirement '{}' configuration template must be a JSON object",
                requirement.key
            ));
        }
        let mut items = 0;
        validate_configuration_template_value(&template, 1, &mut items, &requirement.key)?;
        templates.insert(requirement.key.clone(), template);
    }
    Ok(templates)
}

pub(super) fn validate_configuration_template_value(
    value: &Value,
    depth: usize,
    items: &mut usize,
    requirement_key: &str,
) -> Result<(), SolutionPackError> {
    if depth > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_DEPTH {
        return invalid(format!(
            "extension requirement '{requirement_key}' configuration template exceeds the depth limit"
        ));
    }
    match value {
        Value::Object(object) => {
            *items = items.saturating_add(object.len());
            if *items > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS {
                return invalid(format!(
                    "extension requirement '{requirement_key}' configuration template exceeds the item limit"
                ));
            }
            for (key, value) in object {
                if key.len() > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES {
                    return invalid(format!(
                        "extension requirement '{requirement_key}' configuration template has an oversized key"
                    ));
                }
                let key = key.to_ascii_lowercase();
                if [
                    "password",
                    "secret",
                    "token",
                    "api_key",
                    "private_key",
                    "credential",
                    "authorization",
                ]
                .iter()
                .any(|suffix| key == *suffix || key.ends_with(suffix))
                {
                    return invalid(format!(
                        "extension requirement '{requirement_key}' configuration template contains a secret-like key"
                    ));
                }
                validate_configuration_template_value(value, depth + 1, items, requirement_key)?;
            }
        }
        Value::Array(array) => {
            *items = items.saturating_add(array.len());
            if *items > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS {
                return invalid(format!(
                    "extension requirement '{requirement_key}' configuration template exceeds the item limit"
                ));
            }
            for value in array {
                validate_configuration_template_value(value, depth + 1, items, requirement_key)?;
            }
        }
        Value::String(string)
            if string.len() > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES =>
        {
            return invalid(format!(
                "extension requirement '{requirement_key}' configuration template has an oversized string"
            ));
        }
        _ => {}
    }
    Ok(())
}

pub fn json_deep_contains(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            expected.iter().all(|(key, expected)| {
                actual
                    .get(key)
                    .is_some_and(|actual| json_deep_contains(actual, expected))
            })
        }
        _ => actual == expected,
    }
}
