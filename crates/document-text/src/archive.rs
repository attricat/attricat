//! Office Open XML archives checked against decompression limits before any
//! part of them is parsed.

use std::io::{Cursor, Read};

use zip::ZipArchive;

use crate::{ExtractError, Limits};

/// An archive whose every entry decompresses within the limits.
pub(crate) struct Archive<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
    max_entry_bytes: u64,
}

impl<'a> Archive<'a> {
    /// Opens an archive after decompressing each entry once to measure it. The
    /// sizes an archive declares are not trusted.
    pub(crate) fn open(bytes: &'a [u8], limits: &Limits) -> Result<Self, ExtractError> {
        let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| ExtractError::Malformed)?;
        if zip.len() > limits.max_archive_entries {
            return Err(ExtractError::TooLarge);
        }
        let mut total = 0_u64;
        for index in 0..zip.len() {
            let entry = zip.by_index(index).map_err(|_| ExtractError::Malformed)?;
            let size = measure(entry, limits.max_archive_entry_bytes)?;
            total += size;
            if total > limits.max_archive_total_bytes {
                return Err(ExtractError::TooLarge);
            }
        }
        Ok(Self {
            zip,
            max_entry_bytes: limits.max_archive_entry_bytes,
        })
    }

    /// The decompressed bytes of an entry, or `None` when the archive lacks it.
    pub(crate) fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, ExtractError> {
        let entry = match self.zip.by_name(name) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(_) => return Err(ExtractError::Malformed),
        };
        let mut bytes = Vec::new();
        entry
            .take(self.max_entry_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ExtractError::Malformed)?;
        if bytes.len() as u64 > self.max_entry_bytes {
            return Err(ExtractError::TooLarge);
        }
        Ok(Some(bytes))
    }
}

/// Decompresses an entry without keeping it, failing past `limit` bytes.
fn measure(entry: impl Read, limit: u64) -> Result<u64, ExtractError> {
    let size = std::io::copy(&mut entry.take(limit + 1), &mut std::io::sink())
        .map_err(|_| ExtractError::Malformed)?;
    if size > limit {
        return Err(ExtractError::TooLarge);
    }
    Ok(size)
}
