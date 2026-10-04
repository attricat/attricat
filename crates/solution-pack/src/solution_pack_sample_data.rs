//! Strict format-1 solution-pack sample declarations and the immutable
//! `solution-pack-sample-prohibited-v1` lexical matcher.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::OnceLock,
};

use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub const MAX_SAMPLE_DATA_BYTES: usize = 1024 * 1024;
pub const MAX_SAMPLE_ENTITIES: usize = 256;
pub const MAX_SAMPLE_FACTS_PER_ENTITY: usize = 128;
pub const MAX_SAMPLE_TOTAL_FACTS: usize = 4096;
pub const MAX_SAMPLE_FILES: usize = 64;
pub const MAX_SAMPLE_FILES_PER_VALUE: usize = 16;
pub const MAX_SAMPLE_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SAMPLE_FILENAME_BYTES: usize = 255;
/// Media types a sample may bundle, with the filename extensions each accepts.
pub const SAMPLE_FILE_MEDIA_TYPES: &[(&str, &[&str])] = &[
    ("image/png", &["png"]),
    ("image/jpeg", &["jpg", "jpeg"]),
    ("image/webp", &["webp"]),
    ("application/pdf", &["pdf"]),
    ("text/plain", &["txt"]),
];
pub const SAMPLE_AUTOMATION_WARNING: &str = "Sample entities are ordinary workspace entities. Creating them emits ordinary audit records and entity.created.v1 events, may run enabled automation or extensions, and may cause external effects. Staging cleanup does not remove values retained by ordinary audit or event storage.";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleDataDeclaration {
    pub format_version: u32,
    pub kind: String,
    pub classification: String,
    pub entities: Vec<SampleEntity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleEntity {
    pub key: String,
    pub blueprint: String,
    pub facts: Vec<SampleFact>,
    pub relationships: Vec<SampleRelationship>,
    /// Bundled files attached to file attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SampleFileValue>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFact {
    pub attribute: String,
    pub value: Value,
    /// Pack context key; omitted for the workspace default context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleRelationship {
    pub attribute: String,
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFileValue {
    pub attribute: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub files: Vec<SampleFile>,
}

/// A bundled file declared in the manifest's `sample_data.files`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFile {
    pub path: String,
    pub filename: String,
    pub media_type: String,
}

/// Archive paths of bundled sample files.
pub fn valid_sample_file_path(path: &str) -> bool {
    path.len() <= 512
        && path
            .strip_prefix("sample-data/files/")
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.split('/').all(|component| {
                        let mut bytes = component.bytes();
                        bytes.next().is_some_and(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                        }) && bytes.all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                        })
                    })
            })
}

/// Checks a bundled file's bytes against its declared media type. Sample
/// files are not scanned for content; only their type is verified.
pub fn validate_sample_file_bytes(media_type: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_SAMPLE_FILE_BYTES {
        return Err("sample file must be non-empty and at most 8 MiB".into());
    }
    // The same sniffer as ordinary uploads; sample media types never need
    // the file name to disambiguate.
    let matches = catalog_validation::files::detect_mime(
        bytes,
        "",
        catalog_validation::files::is_plain_text(bytes),
    ) == Some(media_type);
    if matches {
        Ok(())
    } else {
        Err(format!(
            "sample file content does not match media type {media_type}"
        ))
    }
}

fn validate_sample_file(file: &SampleFile, declared_files: &BTreeSet<&str>) -> Result<(), String> {
    if !declared_files.contains(file.path.as_str()) {
        return Err(format!(
            "sample file '{}' is not declared in the manifest",
            file.path
        ));
    }
    let extensions = SAMPLE_FILE_MEDIA_TYPES
        .iter()
        .find(|(media_type, _)| *media_type == file.media_type)
        .map(|(_, extensions)| *extensions)
        .ok_or_else(|| format!("sample file '{}' media type is unsupported", file.path))?;
    // A plain stem keeps file names from carrying URLs, addresses, or paths;
    // the stem is also checked with the ordinary sample-value matcher.
    let name = &file.filename;
    let valid = name.len() <= MAX_SAMPLE_FILENAME_BYTES
        && name.rsplit_once('.').is_some_and(|(stem, extension)| {
            !stem.is_empty()
                && stem.trim() == stem
                && stem.chars().all(|character| {
                    character.is_alphanumeric() || matches!(character, ' ' | '-' | '_' | '(' | ')')
                })
                && extensions.contains(&extension.to_ascii_lowercase().as_str())
        });
    if !valid {
        return Err(format!(
            "sample file '{}' filename must be a plain name with a matching extension",
            file.path
        ));
    }
    let stem = name.rsplit_once('.').map_or("", |(stem, _)| stem);
    prohibited_scalar("filename", stem)
}

#[derive(Clone, Debug)]
pub struct ValidatedSampleData {
    pub declaration: SampleDataDeclaration,
    /// Bundled files by archive path. Every reference to one path uses the
    /// same filename and media type, so it becomes one ordinary file.
    pub files: BTreeMap<String, SampleFile>,
    pub canonical_sha256: String,
    /// Entity indices in deterministic target-before-source order.
    pub target_first_order: Vec<usize>,
    pub scalar_fact_count: usize,
    pub relationship_fact_count: usize,
}

pub fn validate_sample_data(
    bytes: &[u8],
    declared_blueprints: &BTreeSet<&str>,
    declared_contexts: &BTreeSet<&str>,
    declared_files: &BTreeSet<&str>,
) -> Result<ValidatedSampleData, String> {
    if bytes.is_empty() || bytes.len() > MAX_SAMPLE_DATA_BYTES {
        return Err("sample-data file must be non-empty and at most 1 MiB".into());
    }
    let declaration: SampleDataDeclaration = serde_json::from_slice(bytes)
        .map_err(|_| "sample-data file is not valid strict JSON".to_owned())?;
    if declaration.format_version != 1 || declaration.kind != "solution_pack_sample_data" {
        return Err("sample-data format_version and kind are unsupported".into());
    }
    if declaration.classification != "synthetic" {
        return Err("sample-data classification must be synthetic".into());
    }
    if declaration.entities.is_empty() || declaration.entities.len() > MAX_SAMPLE_ENTITIES {
        return Err(format!(
            "sample-data entities must contain 1-{MAX_SAMPLE_ENTITIES} entries"
        ));
    }

    let mut keys = BTreeMap::new();
    let mut files = BTreeMap::<String, SampleFile>::new();
    for (index, entity) in declaration.entities.iter().enumerate() {
        if !valid_sample_entity_key(&entity.key) {
            return Err(format!("sample entity key '{}' is invalid", entity.key));
        }
        if keys.insert(entity.key.as_str(), index).is_some() {
            return Err(format!("duplicate sample entity key '{}'", entity.key));
        }
        if !declared_blueprints.contains(entity.blueprint.as_str()) {
            return Err(format!(
                "sample entity '{}' references an undeclared blueprint",
                entity.key
            ));
        }
        if entity.facts.len() + entity.relationships.len() + entity.files.len()
            > MAX_SAMPLE_FACTS_PER_ENTITY
        {
            return Err(format!(
                "sample entity '{}' exceeds the fact limit",
                entity.key
            ));
        }
        let valid_context = |context: Option<&String>| {
            context.is_none_or(|context| declared_contexts.contains(context.as_str()))
        };
        if !entity
            .facts
            .iter()
            .all(|fact| valid_context(fact.context.as_ref()))
            || !entity
                .relationships
                .iter()
                .all(|relationship| valid_context(relationship.context.as_ref()))
            || !entity
                .files
                .iter()
                .all(|value| valid_context(value.context.as_ref()))
        {
            return Err(format!(
                "sample entity '{}' references an undeclared context",
                entity.key
            ));
        }
        let mut attributes = BTreeSet::new();
        for fact in &entity.facts {
            validate_attribute_reference(&entity.blueprint, &fact.attribute)?;
            if !attributes.insert((fact.attribute.as_str(), fact.context.as_deref())) {
                return Err(format!(
                    "sample entity '{}' has duplicate attribute facts",
                    entity.key
                ));
            }
            if !matches!(
                fact.value,
                Value::String(_) | Value::Bool(_) | Value::Number(_)
            ) && !is_exact_time_object(&fact.value)
            {
                return Err(format!(
                    "sample entity '{}' has an unsupported fact value",
                    entity.key
                ));
            }
            // Value matching is type-aware and runs after the referenced
            // blueprint attribute has been resolved. Structural validation only
            // permits scalars and the one strict native-time object here.
            prohibited_attribute_code(
                fact.attribute.rsplit('/').next().unwrap_or(&fact.attribute),
            )?;
        }
        for relationship in &entity.relationships {
            validate_attribute_reference(&entity.blueprint, &relationship.attribute)?;
            if !attributes.insert((
                relationship.attribute.as_str(),
                relationship.context.as_deref(),
            )) {
                return Err(format!(
                    "sample entity '{}' has duplicate attribute facts",
                    entity.key
                ));
            }
            if relationship.targets.is_empty() {
                return Err(format!(
                    "sample entity '{}' has an empty relationship target list",
                    entity.key
                ));
            }
            let mut targets = BTreeSet::new();
            for target in &relationship.targets {
                if !targets.insert(target.as_str()) {
                    return Err(format!(
                        "sample entity '{}' has duplicate relationship targets",
                        entity.key
                    ));
                }
                if target == &entity.key {
                    return Err(format!(
                        "sample entity '{}' has a self relationship",
                        entity.key
                    ));
                }
            }
        }
        for value in &entity.files {
            validate_attribute_reference(&entity.blueprint, &value.attribute)?;
            if !attributes.insert((value.attribute.as_str(), value.context.as_deref())) {
                return Err(format!(
                    "sample entity '{}' has duplicate attribute facts",
                    entity.key
                ));
            }
            if value.files.is_empty() || value.files.len() > MAX_SAMPLE_FILES_PER_VALUE {
                return Err(format!(
                    "sample entity '{}' file values must list 1-{MAX_SAMPLE_FILES_PER_VALUE} files",
                    entity.key
                ));
            }
            let mut paths = BTreeSet::new();
            for file in &value.files {
                validate_sample_file(file, declared_files)?;
                if !paths.insert(file.path.as_str()) {
                    return Err(format!(
                        "sample entity '{}' attaches a file twice to one value",
                        entity.key
                    ));
                }
                match files.get(file.path.as_str()) {
                    Some(existing)
                        if existing.media_type != file.media_type
                            || existing.filename != file.filename =>
                    {
                        return Err(format!(
                            "sample file '{}' is referenced with different filenames or media types",
                            file.path
                        ));
                    }
                    _ => {
                        files.insert(file.path.clone(), file.clone());
                    }
                }
            }
        }
    }
    if let Some(unused) = declared_files
        .iter()
        .find(|path| !files.contains_key(**path))
    {
        return Err(format!(
            "sample file '{unused}' is not attached by any sample entity"
        ));
    }

    let scalar_fact_count = declaration
        .entities
        .iter()
        .map(|entity| entity.facts.len())
        .sum::<usize>();
    let relationship_fact_count = declaration
        .entities
        .iter()
        .flat_map(|entity| &entity.relationships)
        .map(|relationship| relationship.targets.len())
        .sum::<usize>();
    if scalar_fact_count + relationship_fact_count > MAX_SAMPLE_TOTAL_FACTS {
        return Err(format!(
            "sample-data exceeds the {MAX_SAMPLE_TOTAL_FACTS} total fact limit"
        ));
    }

    let mut dependencies = vec![BTreeSet::new(); declaration.entities.len()];
    for (source, entity) in declaration.entities.iter().enumerate() {
        for relationship in &entity.relationships {
            for target in &relationship.targets {
                let target_index = keys.get(target.as_str()).copied().ok_or_else(|| {
                    format!(
                        "sample entity '{}' references unknown same-file target '{target}'",
                        entity.key
                    )
                })?;
                dependencies[source].insert(target_index);
            }
        }
    }
    let remaining = dependencies;
    let mut target_first_order = Vec::with_capacity(declaration.entities.len());
    while target_first_order.len() < declaration.entities.len() {
        let next = remaining
            .iter()
            .enumerate()
            .filter(|(index, _)| !target_first_order.contains(index))
            .filter(|(_, deps)| deps.iter().all(|dep| target_first_order.contains(dep)))
            .min_by_key(|(index, _)| declaration.entities[*index].key.as_str())
            .map(|(index, _)| index)
            .ok_or_else(|| "sample-data relationships must be acyclic".to_owned())?;
        target_first_order.push(next);
    }

    let canonical = serde_json::to_vec(&declaration).expect("sample declaration serializes");
    Ok(ValidatedSampleData {
        declaration,
        files,
        canonical_sha256: format!("{:x}", Sha256::digest(canonical)),
        target_first_order,
        scalar_fact_count,
        relationship_fact_count,
    })
}

fn is_exact_time_object(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 2
        && object.get("time").and_then(Value::as_str).is_some()
        && object.get("time_zone").and_then(Value::as_str).is_some()
}

pub fn explicit_fact_attribute_codes(entity: &SampleEntity) -> BTreeSet<&str> {
    entity
        .facts
        .iter()
        .map(|fact| fact.attribute.rsplit('/').next().unwrap_or_default())
        .collect()
}

fn valid_sample_entity_key(value: &str) -> bool {
    let Some(code) = value.strip_prefix("sample-entities/") else {
        return false;
    };
    !code.is_empty()
        && code.len() <= 128
        && code.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            }
        })
}

fn validate_attribute_reference(blueprint: &str, attribute: &str) -> Result<(), String> {
    let Some(code) = attribute.strip_prefix(&format!("{blueprint}/attributes/")) else {
        return Err(format!(
            "sample attribute '{attribute}' is not a logical attribute of '{blueprint}'"
        ));
    };
    if code.is_empty()
        || code.len() > 128
        || !code.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            }
        })
    {
        return Err(format!("sample attribute '{attribute}' is invalid"));
    }
    prohibited_attribute_code(code)
}

pub fn prohibited_attribute_code(code: &str) -> Result<(), String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = code.chars().collect();
    for (index, ch) in chars.iter().copied().enumerate() {
        let camel_boundary =
            index > 0 && chars[index - 1].is_ascii_lowercase() && ch.is_ascii_uppercase();
        if ch.is_ascii_punctuation() || ch.is_ascii_whitespace() || camel_boundary {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if camel_boundary {
                current.push(ch);
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    let folded = tokens
        .iter()
        .map(|token| ascii_fold(token))
        .collect::<Vec<_>>();
    let concatenated = folded.concat();
    const PROHIBITED: [&str; 9] = [
        "password",
        "secret",
        "token",
        "apikey",
        "privatekey",
        "credential",
        "authorization",
        "session",
        "cookie",
    ];
    if folded
        .iter()
        .any(|token| PROHIBITED.contains(&token.as_str()))
        || PROHIBITED.contains(&concatenated.as_str())
    {
        Err(format!("sample attribute code '{code}' is prohibited"))
    } else {
        Ok(())
    }
}

pub fn prohibited_scalar(attribute: &str, input: &str) -> Result<(), String> {
    prohibited_attribute_code(attribute.rsplit('/').next().unwrap_or(attribute))?;
    let normalized: String = input.nfc().collect();
    validate_string_basics(&normalized)?;
    scan_prohibited(&normalized, true, true)?;
    if normalized.contains('%') {
        let decoded = percent_decode_once(&normalized)?;
        if residual_percent_escape(&decoded) {
            return Err("sample value contains a residual percent escape".into());
        }
        validate_string_basics(&decoded)?;
        scan_prohibited(&decoded, true, true)?;
    }
    Ok(())
}

pub fn prohibited_temporal_scalar(attribute: &str, input: &str) -> Result<(), String> {
    prohibited_attribute_code(attribute.rsplit('/').next().unwrap_or(attribute))?;
    let normalized: String = input.nfc().collect();
    validate_string_basics(&normalized)
}

pub fn prohibited_numeric(attribute: &str, canonical: &str) -> Result<(), String> {
    prohibited_attribute_code(attribute.rsplit('/').next().unwrap_or(attribute))?;
    if canonical.parse::<f64>().is_ok_and(f64::is_finite) {
        scan_prohibited(canonical, false, true)
    } else {
        Err("sample numeric value must be finite".into())
    }
}

fn validate_string_basics(value: &str) -> Result<(), String> {
    if value.len() > 4096 {
        return Err("sample string exceeds 4096 UTF-8 bytes".into());
    }
    if value.chars().any(|ch| {
        let code = ch as u32;
        ch == '\r'
            || ch == '\n'
            || code <= 0x1f
            || (0x7f..=0x9f).contains(&code)
            || (0xfdd0..=0xfdef).contains(&code)
            || (code & 0xffff == 0xfffe)
            || (code & 0xffff == 0xffff)
    }) {
        return Err("sample string contains prohibited control or noncharacter code points".into());
    }
    Ok(())
}

fn percent_decode_once(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err("sample value contains an invalid percent escape".into());
            }
            let high = hex(bytes[index + 1])
                .ok_or_else(|| "sample value contains an invalid percent escape".to_owned())?;
            let low = hex(bytes[index + 2])
                .ok_or_else(|| "sample value contains an invalid percent escape".to_owned())?;
            output.push(high * 16 + low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| "sample percent-decoded value is not UTF-8".into())
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
fn residual_percent_escape(value: &str) -> bool {
    value
        .as_bytes()
        .windows(3)
        .any(|w| w[0] == b'%' && hex(w[1]).is_some() && hex(w[2]).is_some())
}
fn ascii_fold(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_uppercase() {
                ch.to_ascii_lowercase()
            } else {
                ch
            }
        })
        .collect()
}

struct MatcherPattern {
    regex: Regex,
    ascii_insensitive: bool,
    numeric: bool,
}

fn patterns() -> &'static Vec<MatcherPattern> {
    static PATTERNS: OnceLock<Vec<MatcherPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            (r"(?:^|[^0-9a-z])(?:urn:uuid:)?(?:\{[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})(?:$|[^0-9a-z])", true, true),
            (r"(?:^|[^0-9A-Za-z])[0-9A-HJKMNP-TV-Z]{26}(?:$|[^0-9A-Za-z])", false, true),
            (r"(?:^|[^0-9a-z])[0-9a-f]{24}(?:$|[^0-9a-z])", true, true),
            (r"[a-z0-9!#$%&'*+/=?^_`{|}~-]+@(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}", true, false),
            (r"[a-z][a-z0-9+.-]{0,31}:\S", true, false),
            (r"(?:host|server|data source|dsn)=\S+", true, false),
            (r"-----begin (?:private key|encrypted private key|rsa private key|ec private key|dsa private key|openssh private key)-----", true, false),
            (r"-----end (?:private key|encrypted private key|rsa private key|ec private key|dsa private key|openssh private key)-----", true, false),
            (r"[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", false, false),
            (r"(?:basic|bearer|token)[ \t]+\S+", true, false),
            (r"(?:AKIA|ASIA)[A-Z0-9]{16}", false, false),
            (r"(?:ghp_|gho_|ghu_|ghs_|ghr_)[a-z0-9_]+", true, false),
            (r"sk_live_[a-z0-9_]+", true, false),
            (r"(?:^|[^0-9a-z])[0-9a-f]{32,}(?:$|[^0-9a-z])", true, false),
            (r"\d{4}-\d{2}-\d{2}(?:t\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:z|[+-]\d{2}:\d{2}))?", true, false),
            (r"(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}", true, false),
        ]
        .into_iter()
        .map(|(pattern, ascii_insensitive, numeric)| MatcherPattern {
            regex: Regex::new(pattern).expect("static matcher regex"),
            ascii_insensitive,
            numeric,
        })
        .collect()
    })
}

fn scan_prohibited(value: &str, string_rules: bool, telephone: bool) -> Result<(), String> {
    let folded = ascii_fold(value);
    if patterns().iter().any(|pattern| {
        (string_rules || pattern.numeric)
            && pattern.regex.is_match(if pattern.ascii_insensitive {
                &folded
            } else {
                value
            })
    }) {
        return Err("sample value matches solution-pack-sample-prohibited-v1".into());
    }
    if telephone && contains_telephone(value) {
        return Err("sample value contains a telephone number".into());
    }
    if string_rules {
        if value.starts_with('/')
            || value.starts_with("~/")
            || value.starts_with("./")
            || value.starts_with("../")
            || value.starts_with("\\\\")
            || (value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && value.as_bytes().get(1..3) == Some(b":\\"))
        {
            return Err("sample value contains a path".into());
        }
        if contains_ip_literal(value) {
            return Err("sample value contains an IP address".into());
        }
        if contains_encoded_blob(value) {
            return Err("sample value contains an encoded blob".into());
        }
    } else if contains_encoded_blob(value) {
        return Err("sample numeric value contains an encoded blob".into());
    }
    Ok(())
}

fn contains_telephone(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if bytes[index] == b'+' {
            index += 1;
        }
        while index < bytes.len()
            && (bytes[index].is_ascii_digit()
                || matches!(bytes[index], b' ' | b'.' | b'-' | b'(' | b')'))
        {
            index += 1;
        }
        if index > start {
            let candidate = &value[start..index];
            let digits = candidate.bytes().filter(u8::is_ascii_digit).count();
            let before_ok = start == 0 || !bytes[start - 1].is_ascii_digit();
            let after_ok = index == bytes.len() || !bytes[index].is_ascii_digit();
            if (7..=15).contains(&digits) && before_ok && after_ok {
                return true;
            }
        }
        index = index.max(start + 1);
    }
    false
}

fn contains_ip_literal(value: &str) -> bool {
    const MAX_IPV6_LITERAL_BYTES: usize = 45;
    let bytes = value.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_hexdigit() {
            continue;
        }
        let limit = bytes.len().min(start + MAX_IPV6_LITERAL_BYTES);
        for end in (start + 1)..=limit {
            let byte = bytes[end - 1];
            if !(byte.is_ascii_hexdigit() || matches!(byte, b'.' | b':')) {
                break;
            }
            let candidate =
                std::str::from_utf8(&bytes[start..end]).expect("IP candidate bytes are ASCII");
            if (candidate.contains('.') && Ipv4Addr::from_str(candidate).is_ok())
                || (candidate.contains(':') && Ipv6Addr::from_str(candidate).is_ok())
            {
                return true;
            }
        }
    }
    false
}

fn contains_encoded_blob(value: &str) -> bool {
    // Any longer valid base64/base64url candidate that decodes to at least 24
    // bytes contains a 32-byte unpadded prefix that decodes to exactly 24
    // bytes. Fixed windows make true substring matching linear and bounded by
    // the global 4096-byte string limit.
    const MIN_ENCODED_BLOB_BYTES: usize = 32;
    value
        .as_bytes()
        .windows(MIN_ENCODED_BLOB_BYTES)
        .any(|bytes| {
            let standard = bytes
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'));
            let url_safe = bytes
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
            (standard
                && general_purpose::STANDARD_NO_PAD
                    .decode(bytes)
                    .is_ok_and(|decoded| decoded.len() >= 24))
                || (url_safe
                    && general_purpose::URL_SAFE_NO_PAD
                        .decode(bytes)
                        .is_ok_and(|decoded| decoded.len() >= 24))
        })
}

/// Normalizes one unique-key component with the workspace's shared rule
/// ([`catalog_validation::unique_key::normalize_key_component`]). Stored
/// date-times are already canonical UTC, so sample date-times are converted
/// to that form first.
fn sample_key_component(value: &Value, value_type: &str, case_sensitive: bool) -> Option<Value> {
    let canonical = match (value_type, value) {
        ("datetime", Value::String(text)) => chrono::DateTime::parse_from_rfc3339(text)
            .map(|instant| Value::String(instant.with_timezone(&chrono::Utc).to_rfc3339()))
            .ok(),
        _ => None,
    };
    catalog_validation::unique_key::normalize_key_component(
        value_type,
        canonical.as_ref().unwrap_or(value),
        case_sensitive,
    )
}

/// One sample's context path: the context itself, its pack parents, and the
/// workspace default context (`None`).
type SampleContextPath<'a> = (Option<&'a str>, Vec<Option<&'a str>>);

/// Rejects samples that would collide on one of their blueprint's unique
/// keys, so the dataset cannot fail part-way through an application. Keys
/// are compared like the workspace does; existing workspace entities of a
/// mapped or reused blueprint can still collide when the plan is applied.
pub(crate) fn validate_sample_unique_keys(
    sample: &ValidatedSampleData,
    blueprints: &BTreeMap<String, crate::solution_packs::SolutionPackBlueprint>,
    contexts: &BTreeMap<String, crate::solution_pack_seeds::SeedContext>,
) -> Result<(), String> {
    let mut paths: Vec<SampleContextPath<'_>> = vec![(None, vec![None])];
    for key in contexts.keys() {
        let mut path = Vec::new();
        let mut current = Some(key.as_str());
        while let Some(context) = current {
            path.push(Some(context));
            current = contexts
                .get(context)
                .and_then(|context| context.parent.as_deref());
        }
        path.push(None);
        paths.push((Some(key.as_str()), path));
    }
    let mut seen = HashMap::<(&str, &str, Option<&str>, Vec<Value>), &str>::new();
    for entity in &sample.declaration.entities {
        let Some(blueprint) = blueprints.get(&entity.blueprint) else {
            continue;
        };
        let attributes = blueprint
            .effective_attributes()
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect::<HashMap<_, _>>();
        let mut values = HashMap::<(&str, Option<&str>), Value>::new();
        for fact in &entity.facts {
            let code = fact.attribute.rsplit('/').next().unwrap_or_default();
            values.insert((code, fact.context.as_deref()), fact.value.clone());
        }
        for relationship in &entity.relationships {
            let code = relationship
                .attribute
                .rsplit('/')
                .next()
                .unwrap_or_default();
            if let Some(target) = relationship.targets.iter().min() {
                values.insert(
                    (code, relationship.context.as_deref()),
                    Value::String(target.clone()),
                );
            }
        }
        for key in blueprint.unique_keys() {
            let scoped = if key.scope == "context" {
                &paths[..]
            } else {
                &paths[..1]
            };
            'context: for (context, path) in scoped {
                let mut components = Vec::with_capacity(key.attributes.len());
                for code in &key.attributes {
                    let Some(attribute) = attributes.get(code.as_str()) else {
                        continue 'context;
                    };
                    let mut found = None;
                    for (index, source) in path.iter().enumerate() {
                        if let Some(value) = values.get(&(code.as_str(), *source)) {
                            if index > 0 && attribute.context_fallback == "none" {
                                break;
                            }
                            found = Some(value.clone());
                            break;
                        }
                    }
                    // Defaults are written in the default context.
                    let found = found.or_else(|| {
                        attribute
                            .default_value
                            .clone()
                            .filter(|_| path.len() == 1 || attribute.context_fallback != "none")
                    });
                    let Some(component) = found.and_then(|value| {
                        sample_key_component(&value, &attribute.value_type, key.case_sensitive)
                    }) else {
                        continue 'context;
                    };
                    components.push(component);
                }
                let family = blueprint.key();
                if let Some(other) = seen.insert(
                    (family, key.code.as_str(), *context, components),
                    entity.key.as_str(),
                ) {
                    return Err(format!(
                        "sample entities '{other}' and '{}' share unique key '{}' of '{family}'{}",
                        entity.key,
                        key.code,
                        context.map_or_else(String::new, |context| format!(" in '{context}'"))
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matcher_v1_canonical_corpus() {
        let rejected = [
            "Jane@example.com",
            "+1 (415) 555-0123",
            "https://example.com",
            "{550e8400-e29b-41d4-a716-446655440000}",
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "-----BEGIN PRIVATE KEY-----",
            "AKIAIOSFODNN7EXAMPLE",
            "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB",
            "0123456789abcdef0123456789abcdef",
            "2026-09-18T09:00:00Z",
        ];
        for seed in rejected {
            for candidate in [seed.to_owned(), format!("prefix {seed} suffix")] {
                assert!(
                    prohibited_scalar("description", &candidate).is_err(),
                    "accepted {candidate}"
                );
            }
            // Calling the same immutable matcher for a materialized default is
            // source-independent; archive planning supplies both explicit and
            // inherited/defaulted values through this path.
            assert!(
                prohibited_scalar("description", seed).is_err(),
                "accepted defaulted {seed}"
            );
            let encoded = seed
                .as_bytes()
                .iter()
                .map(|byte| format!("%{byte:02X}"))
                .collect::<String>();
            assert!(
                prohibited_scalar("description", &encoded).is_err(),
                "accepted encoded {seed}"
            );
            let doubled = encoded.replace('%', "%25");
            assert!(
                prohibited_scalar("description", &doubled).is_err(),
                "accepted double encoded {seed}"
            );
        }
        for seed in [
            "Sample Navy Shirt",
            "SKU-1001",
            "abcdefabcdefabcdefabcdefabcdefa",
            "123 456",
        ] {
            assert!(
                prohibited_scalar("description", seed).is_ok(),
                "rejected {seed}"
            );
        }
        for seed in [
            "Jane@example.com",
            "https://example.com",
            "{550e8400-e29b-41d4-a716-446655440000}",
            "-----BEGIN PRIVATE KEY-----",
            "2026-09-18T09:00:00Z",
        ] {
            let alternating = seed
                .chars()
                .enumerate()
                .map(|(index, ch)| {
                    if index % 2 == 0 {
                        ch.to_ascii_uppercase()
                    } else {
                        ch.to_ascii_lowercase()
                    }
                })
                .collect::<String>();
            for candidate in [
                seed.to_ascii_lowercase(),
                seed.to_ascii_uppercase(),
                alternating,
            ] {
                assert!(
                    prohibited_scalar("description", &candidate).is_err(),
                    "accepted case transform {candidate}"
                );
            }
        }
        let json_escaped: String = serde_json::from_str(r#""Jane\u0040example\u002ecom""#).unwrap();
        assert!(prohibited_scalar("description", &json_escaped).is_err());
        for code in ["api_key", "api-key", "apiKey"] {
            assert!(prohibited_attribute_code(code).is_err());
        }
    }

    #[test]
    fn matcher_v1_ip_unicode_and_base64_boundaries() {
        let valid_standard = general_purpose::STANDARD.encode(vec![b'Q'; 24]);
        let valid_url = general_purpose::URL_SAFE_NO_PAD.encode(vec![0xff; 24]);
        for seed in [
            "\"192.0.2.1\"".to_owned(),
            "source=192.0.2.1".to_owned(),
            "dead192.0.2.1beef".to_owned(),
            "2001:0:0:0:0:0:0:1".to_owned(),
            format!("x{valid_standard}"),
            format!("A{valid_url}Z"),
            "ſabcdefabcdefabcdefabcdefſ".to_owned(),
            "000000002026-09-18".to_owned(),
            "x2026-09-18y".to_owned(),
            "ſ2026-09-18€".to_owned(),
            "000000002026-09-18T09:00:00Z9".to_owned(),
        ] {
            assert!(
                prohibited_scalar("description", &seed).is_err(),
                "accepted {seed}"
            );
            let encoded = seed
                .as_bytes()
                .iter()
                .map(|byte| format!("%{byte:02X}"))
                .collect::<String>();
            assert!(
                prohibited_scalar("description", &encoded).is_err(),
                "accepted encoded {seed}"
            );
            assert!(
                prohibited_scalar("description", &encoded.replace('%', "%25")).is_err(),
                "accepted double encoded {seed}"
            );
        }
        assert!(prohibited_scalar("description", "a€").is_ok());
        assert!(prohibited_scalar("description", &valid_standard).is_err());
        assert!(prohibited_scalar("description", &valid_url).is_err());
        let below_threshold = general_purpose::STANDARD.encode(vec![b'Q'; 23]);
        assert!(prohibited_scalar("description", &below_threshold).is_ok());
        for invalid in [
            format!("{}={}", "Q".repeat(16), "Q".repeat(16)),
            format!("{}+_", "Q".repeat(30)),
        ] {
            assert!(
                prohibited_scalar("description", &invalid).is_ok(),
                "invalid base64 widened rejection boundary: {invalid}"
            );
        }
    }

    #[test]
    fn strict_sample_schema_orders_targets_and_rejects_cycles() {
        let blueprints = BTreeSet::from(["blueprints/product"]);
        let valid = serde_json::json!({
            "format_version": 1,
            "kind": "solution_pack_sample_data",
            "classification": "synthetic",
            "entities": [
                {"key":"sample-entities/source","blueprint":"blueprints/product","facts":[],"relationships":[{"attribute":"blueprints/product/attributes/related","targets":["sample-entities/target"]}]},
                {"key":"sample-entities/target","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":"Sample target"}],"relationships":[]}
            ]
        });
        let validated = validate_sample_data(
            &serde_json::to_vec(&valid).unwrap(),
            &blueprints,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(validated.target_first_order, vec![1, 0]);

        let mut cyclic = valid;
        cyclic["entities"][1]["relationships"] = serde_json::json!([{
            "attribute":"blueprints/product/attributes/related",
            "targets":["sample-entities/source"]
        }]);
        assert!(
            validate_sample_data(
                &serde_json::to_vec(&cyclic).unwrap(),
                &blueprints,
                &BTreeSet::new(),
                &BTreeSet::new()
            )
            .unwrap_err()
            .contains("acyclic")
        );
    }

    #[test]
    fn strict_sample_schema_rejects_unknown_fields_and_non_scalars() {
        let blueprints = BTreeSet::from(["blueprints/product"]);
        let native_time = serde_json::json!({
            "format_version":1,
            "kind":"solution_pack_sample_data",
            "classification":"synthetic",
            "entities":[{"key":"sample-entities/a","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/available_at","value":{"time":"12:34:56","time_zone":"UTC"}}],"relationships":[]}]
        });
        validate_sample_data(
            &serde_json::to_vec(&native_time).unwrap(),
            &blueprints,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        for entity in [
            serde_json::json!({"key":"sample-entities/a","blueprint":"blueprints/product","facts":[],"relationships":[],"context":"default"}),
            serde_json::json!({"key":"sample-entities/a","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":{"copied":true}}],"relationships":[]}),
            serde_json::json!({"key":"sample-entities/a","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":{"time":"12:00:00","time_zone":"UTC","extra":true}}],"relationships":[]}),
        ] {
            let input = serde_json::json!({"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[entity]});
            assert!(
                validate_sample_data(
                    &serde_json::to_vec(&input).unwrap(),
                    &blueprints,
                    &BTreeSet::new(),
                    &BTreeSet::new()
                )
                .is_err()
            );
        }
    }
}
