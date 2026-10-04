//! Presentation asset validation: raster images are decoded and checked, and
//! SVG documents are sanitized to a static, self-contained subset.

use std::{collections::BTreeMap, io::Cursor};

use image::{GenericImageView, ImageFormat, ImageReader, Limits};
use quick_xml::{Reader as XmlReader, Writer as XmlWriter, events::Event};

use super::{
    MAX_IDENTIFIER_BYTES, MAX_SOLUTION_PACK_ASSET_DIMENSION, MAX_SOLUTION_PACK_ASSET_PIXELS,
    MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES, MAX_SOLUTION_PACK_PRESENTATION_ASSET_TOTAL_BYTES,
    MAX_SOLUTION_PACK_SVG_BYTES, NormalizedPresentationAsset, SolutionPackError,
    SolutionPackManifest, SolutionPackPresentationAssetResource, ValidatedPresentationAsset,
    invalid, is_valid_stable_code, parse_sha256, safe_archive_path, sha256_hex,
};

fn valid_presentation_asset_path(path: &str) -> bool {
    path.len() <= 512
        && safe_archive_path(path)
        && path.strip_prefix("assets/").is_some_and(|suffix| {
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

pub(super) fn validate_presentation_asset_resource(
    resource: &SolutionPackPresentationAssetResource,
) -> Result<(), SolutionPackError> {
    let Some(code) = resource.key.strip_prefix("assets/") else {
        return invalid(format!(
            "presentation asset key '{}' must be in the assets/ namespace",
            resource.key
        ));
    };
    if resource.key.len() > MAX_IDENTIFIER_BYTES
        || code.contains('/')
        || !is_valid_stable_code(code)
    {
        return invalid(format!(
            "presentation asset key '{}' is invalid",
            resource.key
        ));
    }
    if !valid_presentation_asset_path(&resource.path) {
        return invalid(format!(
            "presentation asset path '{}' is invalid",
            resource.path
        ));
    }
    if !matches!(resource.purpose.as_str(), "logo" | "icon" | "illustration") {
        return invalid(format!(
            "presentation asset '{}' purpose is unsupported",
            resource.key
        ));
    }
    let supported_media = matches!(
        resource.media_type.as_str(),
        "image/png" | "image/jpeg" | "image/webp" | "image/svg+xml"
    );
    if !supported_media
        || (resource.media_type == "image/jpeg" && resource.purpose != "illustration")
    {
        return invalid(format!(
            "presentation asset '{}' media type is not allowed for its purpose",
            resource.key
        ));
    }
    parse_sha256(&resource.sha256).map_err(|()| {
        SolutionPackError::Invalid(format!(
            "presentation asset '{}' sha256 must be 64 lowercase hexadecimal characters",
            resource.key
        ))
    })?;
    Ok(())
}

pub(super) fn validate_presentation_assets(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, ValidatedPresentationAsset>, SolutionPackError> {
    let total_bytes =
        manifest
            .resources
            .presentation_assets
            .iter()
            .try_fold(0usize, |total, resource| {
                let size = files[&resource.path].len();
                if size == 0 || size > MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES {
                    return invalid(format!(
                        "presentation asset '{}' exceeds the per-file size limit",
                        resource.key
                    ));
                }
                total.checked_add(size).ok_or_else(|| {
                    SolutionPackError::Invalid(
                        "presentation asset declared bytes exceed the aggregate size limit".into(),
                    )
                })
            })?;
    if total_bytes > MAX_SOLUTION_PACK_PRESENTATION_ASSET_TOTAL_BYTES {
        return invalid("presentation asset declared bytes exceed the aggregate size limit");
    }

    let mut validated = BTreeMap::new();
    for resource in &manifest.resources.presentation_assets {
        let source = &files[&resource.path];
        let normalized = validate_presentation_asset_bytes(
            &resource.purpose,
            &resource.media_type,
            source,
            &resource.key,
        )?;
        if normalized.bytes.is_empty()
            || normalized.bytes.len() > MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES
        {
            return invalid(format!(
                "presentation asset '{}' sanitized bytes exceed the size limit",
                resource.key
            ));
        }
        let asset = ValidatedPresentationAsset {
            key: resource.key.clone(),
            purpose: resource.purpose.clone(),
            media_type: resource.media_type.clone(),
            source_sha256: resource.sha256.clone(),
            source_byte_size: source.len(),
            stored_sha256: sha256_hex(&normalized.bytes),
            stored_bytes: normalized.bytes,
            width: normalized.width,
            height: normalized.height,
        };
        validated.insert(resource.key.clone(), asset);
    }
    Ok(validated)
}

pub fn validate_presentation_asset_bytes(
    purpose: &str,
    media_type: &str,
    source: &[u8],
    label: &str,
) -> Result<NormalizedPresentationAsset, SolutionPackError> {
    if source.is_empty() || source.len() > MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES {
        return invalid(format!(
            "presentation asset '{label}' exceeds the per-file size limit"
        ));
    }
    if !matches!(purpose, "logo" | "icon" | "illustration")
        || !matches!(
            media_type,
            "image/png" | "image/jpeg" | "image/webp" | "image/svg+xml"
        )
        || (media_type == "image/jpeg" && purpose != "illustration")
    {
        return invalid(format!(
            "presentation asset '{label}' media type is not allowed for its purpose"
        ));
    }
    if media_type == "image/svg+xml" {
        if source.len() > MAX_SOLUTION_PACK_SVG_BYTES {
            return invalid(format!(
                "presentation asset '{label}' SVG exceeds the size limit"
            ));
        }
        return Ok(NormalizedPresentationAsset {
            bytes: sanitize_svg(source, label)?,
            width: None,
            height: None,
        });
    }
    let (format, magic_valid) = match media_type {
        "image/png" => (
            ImageFormat::Png,
            source.starts_with(b"\x89PNG\r\n\x1a\n") && !png_has_animation(source),
        ),
        "image/jpeg" => (
            ImageFormat::Jpeg,
            source.starts_with(&[0xff, 0xd8]) && source.ends_with(&[0xff, 0xd9]),
        ),
        "image/webp" => (ImageFormat::WebP, valid_static_webp(source)),
        _ => unreachable!("media type allowlisted above"),
    };
    if !magic_valid {
        return invalid(format!(
            "presentation asset '{label}' has invalid or animated media content"
        ));
    }
    // Read dimensions from format metadata and reject unsafe output geometry before any
    // decoder is allowed to allocate the fully decompressed pixel buffer.
    let (width, height) = ImageReader::with_format(Cursor::new(source), format)
        .into_dimensions()
        .map_err(|_| {
            SolutionPackError::Invalid(format!(
                "presentation asset '{label}' dimensions cannot be inspected"
            ))
        })?;
    if width == 0
        || height == 0
        || width > MAX_SOLUTION_PACK_ASSET_DIMENSION
        || height > MAX_SOLUTION_PACK_ASSET_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_SOLUTION_PACK_ASSET_PIXELS
    {
        return invalid(format!(
            "presentation asset '{label}' dimensions exceed the limit"
        ));
    }
    let mut reader = ImageReader::with_format(Cursor::new(source), format);
    let mut decode_limits = Limits::default();
    decode_limits.max_image_width = Some(MAX_SOLUTION_PACK_ASSET_DIMENSION);
    decode_limits.max_image_height = Some(MAX_SOLUTION_PACK_ASSET_DIMENSION);
    // The output is bounded to 64 MiB; retain bounded headroom for decoder scratch space.
    decode_limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(decode_limits);
    let image = reader.decode().map_err(|_| {
        SolutionPackError::Invalid(format!(
            "presentation asset '{label}' cannot be completely decoded as its declared media type"
        ))
    })?;
    if image.dimensions() != (width, height) {
        return invalid(format!(
            "presentation asset '{label}' decoded dimensions are inconsistent"
        ));
    }
    Ok(NormalizedPresentationAsset {
        bytes: source.to_vec(),
        width: Some(width),
        height: Some(height),
    })
}

fn png_has_animation(bytes: &[u8]) -> bool {
    let mut offset = 8usize;
    while offset.checked_add(12).is_some_and(|end| end <= bytes.len()) {
        let length =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("four bytes")) as usize;
        let Some(end) = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
        else {
            return true;
        };
        if end > bytes.len() {
            return true;
        }
        if &bytes[offset + 4..offset + 8] == b"acTL" {
            return true;
        }
        offset = end;
    }
    false
}

fn valid_static_webp(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }
    let declared = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes")) as usize;
    declared.checked_add(8) == Some(bytes.len())
        && !bytes
            .windows(4)
            .any(|chunk| chunk == b"ANIM" || chunk == b"ANMF")
}

fn valid_svg_fragment_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn local_svg_fragment(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(valid_svg_fragment_id)
}

fn local_svg_url(value: &str) -> bool {
    value
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
        .is_some_and(valid_svg_fragment_id)
}

fn primitive_svg_paint(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.contains('\\') || !value.is_ascii() {
        return false;
    }
    if matches!(
        value,
        "none" | "currentColor" | "transparent" | "context-fill" | "context-stroke"
    ) || local_svg_url(value)
    {
        return true;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
    {
        return true;
    }
    for function in ["rgb(", "rgba(", "hsl(", "hsla("] {
        if let Some(arguments) = value
            .strip_prefix(function)
            .and_then(|arguments| arguments.strip_suffix(')'))
        {
            return !arguments.is_empty()
                && arguments.bytes().all(|byte| {
                    byte.is_ascii_digit()
                        || matches!(byte, b' ' | b',' | b'.' | b'%' | b'/' | b'+' | b'-')
                });
        }
    }
    false
}

fn safe_svg_attribute_value(name: &str, value: &str) -> bool {
    match name {
        "href" | "xlink:href" => !value.contains('\\') && local_svg_fragment(value),
        "clip-path" | "mask" => {
            !value.contains('\\') && (value.trim() == "none" || local_svg_url(value.trim()))
        }
        "fill" | "stroke" | "stop-color" => primitive_svg_paint(value),
        _ => true,
    }
}

pub(super) fn sanitize_svg(source: &[u8], key: &str) -> Result<Vec<u8>, SolutionPackError> {
    let text = std::str::from_utf8(source).map_err(|_| {
        SolutionPackError::Invalid(format!("presentation asset '{key}' SVG must be UTF-8"))
    })?;
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return invalid(format!(
            "presentation asset '{key}' SVG contains forbidden declarations"
        ));
    }

    const ELEMENTS: &[&str] = &[
        "svg",
        "g",
        "path",
        "rect",
        "circle",
        "ellipse",
        "line",
        "polyline",
        "polygon",
        "defs",
        "linearGradient",
        "radialGradient",
        "stop",
        "clipPath",
        "mask",
        "title",
        "desc",
        "use",
        "symbol",
    ];
    const ATTRIBUTES: &[&str] = &[
        "xmlns",
        "xmlns:xlink",
        "viewBox",
        "width",
        "height",
        "x",
        "y",
        "x1",
        "y1",
        "x2",
        "y2",
        "cx",
        "cy",
        "r",
        "rx",
        "ry",
        "d",
        "points",
        "fill",
        "fill-rule",
        "fill-opacity",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
        "stroke-miterlimit",
        "stroke-dasharray",
        "stroke-dashoffset",
        "stroke-opacity",
        "opacity",
        "transform",
        "gradientUnits",
        "gradientTransform",
        "offset",
        "stop-color",
        "stop-opacity",
        "clip-path",
        "clip-rule",
        "mask",
        "id",
        "preserveAspectRatio",
        "href",
        "xlink:href",
    ];

    let mut reader = XmlReader::from_str(text);
    reader.config_mut().trim_text(false);
    let mut writer = XmlWriter::new(Vec::with_capacity(source.len()));
    let mut depth = 0usize;
    let mut elements = 0usize;
    loop {
        let event = reader.read_event().map_err(|_| {
            SolutionPackError::Invalid(format!("presentation asset '{key}' SVG is malformed"))
        })?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let name = std::str::from_utf8(start.name().as_ref())
                    .map_err(|_| {
                        SolutionPackError::Invalid(format!(
                            "presentation asset '{key}' SVG has an invalid element"
                        ))
                    })?
                    .to_owned();
                if name.contains(':')
                    || !ELEMENTS.contains(&name.as_str())
                    || (depth == 0 && (name != "svg" || elements != 0))
                {
                    return invalid(format!(
                        "presentation asset '{key}' SVG contains forbidden elements"
                    ));
                }
                elements += 1;
                if elements > 4096 || depth > 64 {
                    return invalid(format!("presentation asset '{key}' SVG is too complex"));
                }
                let mut attrs = Vec::new();
                for attribute in start.attributes() {
                    let attribute = attribute.map_err(|_| {
                        SolutionPackError::Invalid(format!(
                            "presentation asset '{key}' SVG has malformed attributes"
                        ))
                    })?;
                    let attr_name = std::str::from_utf8(attribute.key.as_ref()).map_err(|_| {
                        SolutionPackError::Invalid(format!(
                            "presentation asset '{key}' SVG has an invalid attribute"
                        ))
                    })?;
                    if attr_name.starts_with("on") || !ATTRIBUTES.contains(&attr_name) {
                        return invalid(format!(
                            "presentation asset '{key}' SVG contains forbidden attributes"
                        ));
                    }
                    let value = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| {
                            SolutionPackError::Invalid(format!(
                                "presentation asset '{key}' SVG has malformed attribute values"
                            ))
                        })?
                        .into_owned();
                    let namespace = matches!(attr_name, "xmlns" | "xmlns:xlink");
                    if attr_name == "xmlns" && value != "http://www.w3.org/2000/svg"
                        || attr_name == "xmlns:xlink" && value != "http://www.w3.org/1999/xlink"
                    {
                        return invalid(format!(
                            "presentation asset '{key}' SVG contains an unknown namespace"
                        ));
                    }
                    if !namespace && !safe_svg_attribute_value(attr_name, &value) {
                        return invalid(format!(
                            "presentation asset '{key}' SVG contains active, remote, or invalid attribute content"
                        ));
                    }
                    attrs.push((attr_name.to_owned(), value));
                }
                if depth == 0
                    && !attrs.iter().any(|(name, value)| {
                        name == "xmlns" && value == "http://www.w3.org/2000/svg"
                    })
                {
                    return invalid(format!(
                        "presentation asset '{key}' SVG root namespace is invalid"
                    ));
                }
                attrs.sort();
                let mut normalized = quick_xml::events::BytesStart::new(&name);
                for (name, value) in &attrs {
                    normalized.push_attribute((name.as_str(), value.as_str()));
                }
                if empty {
                    writer.write_event(Event::Empty(normalized))
                } else {
                    writer.write_event(Event::Start(normalized))
                }
                .map_err(|_| {
                    SolutionPackError::Invalid(format!(
                        "presentation asset '{key}' SVG cannot be normalized"
                    ))
                })?;
                if !empty {
                    depth += 1;
                }
            }
            Event::End(end) => {
                if depth == 0 {
                    return invalid(format!("presentation asset '{key}' SVG is malformed"));
                }
                depth -= 1;
                writer
                    .write_event(Event::End(end.into_owned()))
                    .map_err(|_| {
                        SolutionPackError::Invalid(format!(
                            "presentation asset '{key}' SVG cannot be normalized"
                        ))
                    })?;
            }
            Event::Text(text) => {
                let text_bytes: &[u8] = text.as_ref();
                if depth == 0 && !text_bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
                    return invalid(format!(
                        "presentation asset '{key}' SVG has text outside its root"
                    ));
                }
                writer
                    .write_event(Event::Text(text.into_owned()))
                    .map_err(|_| {
                        SolutionPackError::Invalid(format!(
                            "presentation asset '{key}' SVG cannot be normalized"
                        ))
                    })?
            }
            Event::Eof => break,
            Event::Decl(_)
            | Event::DocType(_)
            | Event::PI(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::GeneralRef(_) => {
                return invalid(format!(
                    "presentation asset '{key}' SVG contains forbidden XML content"
                ));
            }
        }
    }
    if depth != 0 || elements == 0 {
        return invalid(format!("presentation asset '{key}' SVG is malformed"));
    }
    let output = writer.into_inner();
    if output.len() > MAX_SOLUTION_PACK_SVG_BYTES {
        return invalid(format!(
            "presentation asset '{key}' sanitized SVG exceeds the size limit"
        ));
    }
    Ok(output)
}
