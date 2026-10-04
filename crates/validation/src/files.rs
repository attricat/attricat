//! File type detection and attribute file-policy checks shared by uploads and
//! the files that solution-pack samples bundle.

const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

/// MIME types accepted by every upload entry point. Attribute policies may
/// further restrict this set by MIME group, extension, size, or image-only.
pub const SUPPORTED_UPLOAD_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "application/pdf",
    "text/plain",
    DOCX_MIME,
    XLSX_MIME,
    PPTX_MIME,
];

pub fn is_supported_upload_mime(mime: &str) -> bool {
    SUPPORTED_UPLOAD_MIME_TYPES.contains(&mime)
}

/// The lowercase extension of a file name, if any.
pub fn extension(filename: &str) -> Option<String> {
    filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

/// Detects a supported MIME type from the leading bytes of a file. The file
/// name only disambiguates Office Open XML containers. `valid_text` says
/// whether the whole stream is UTF-8 text without NUL bytes.
pub fn detect_mime(signature: &[u8], filename: &str, valid_text: bool) -> Option<&'static str> {
    if signature.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if signature.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if signature.starts_with(b"GIF87a") || signature.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if signature.len() >= 12 && &signature[..4] == b"RIFF" && &signature[8..12] == b"WEBP" {
        Some("image/webp")
    } else if signature.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if signature.starts_with(b"PK\x03\x04") {
        match extension(filename).as_deref() {
            Some("docx") => Some(DOCX_MIME),
            Some("xlsx") => Some(XLSX_MIME),
            Some("pptx") => Some(PPTX_MIME),
            _ => None,
        }
    } else if valid_text {
        Some("text/plain")
    } else {
        None
    }
}

/// Whether complete in-memory bytes are UTF-8 text without NUL bytes.
pub fn is_plain_text(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|text| !text.contains('\0'))
}

/// The restrictions of an attribute's file policy.
#[derive(Clone, Copy, Debug)]
pub struct FileConstraints<'a> {
    pub allowed_mime_groups: &'a [String],
    pub allowed_extensions: &'a [String],
    pub max_bytes: Option<u64>,
    pub image_only: bool,
}

impl FileConstraints<'_> {
    /// Whether a file of a supported, detected MIME type is accepted.
    pub fn allows(&self, mime: &str, filename: &str, size: u64) -> bool {
        if !is_supported_upload_mime(mime)
            || self.max_bytes.is_some_and(|limit| size > limit)
            || (self.image_only && !mime.starts_with("image/"))
        {
            return false;
        }
        let family = mime.split('/').next().unwrap_or_default();
        if !self.allowed_mime_groups.is_empty()
            && !self.allowed_mime_groups.iter().any(|group| {
                group == mime || group.trim_end_matches("/*") == family || group == family
            })
        {
            return false;
        }
        self.allowed_extensions.is_empty()
            || extension(filename).is_some_and(|extension| {
                self.allowed_extensions.iter().any(|allowed| {
                    allowed
                        .trim_start_matches('.')
                        .eq_ignore_ascii_case(&extension)
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_constrains_type_extension_and_size() {
        let groups = vec!["image/*".to_owned()];
        let extensions = vec!["png".to_owned()];
        let policy = FileConstraints {
            allowed_mime_groups: &groups,
            allowed_extensions: &extensions,
            max_bytes: Some(1024),
            image_only: true,
        };
        assert!(policy.allows("image/png", "photo.PNG", 1024));
        assert!(!policy.allows("image/jpeg", "photo.jpg", 1));
        assert!(!policy.allows("image/png", "photo.png", 1025));
        assert!(!policy.allows("image/svg+xml", "photo.png", 1));
    }

    #[test]
    fn detects_signatures_without_trusting_file_names() {
        assert_eq!(
            detect_mime(b"\x89PNG\r\n\x1a\nrest", "x.txt", false),
            Some("image/png")
        );
        assert_eq!(
            detect_mime(b"plain", "x.png", is_plain_text(b"plain")),
            Some("text/plain")
        );
        assert_eq!(
            detect_mime(b"bin\0", "x.png", is_plain_text(b"bin\0")),
            None
        );
    }
}
