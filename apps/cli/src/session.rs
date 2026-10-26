//! Browser-session cookies persisted for the auth commands.

use std::{
    fs,
    io::{self},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::error::CliError;
use crate::transport::{persist_temporary_file, raw_response, temporary_file};

pub(crate) const SESSION_COOKIE: &str = "attricat_session";

pub(crate) const CSRF_COOKIE: &str = "attricat_csrf";

pub(crate) const CSRF_HEADER: &str = "x-attricat-csrf";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionFile {
    pub(crate) session: String,
    pub(crate) csrf: String,
}

impl SessionFile {
    pub(crate) fn load(path: Option<&PathBuf>) -> Result<Option<Self>, CliError> {
        path.map(|path| match fs::read_to_string(path) {
            Ok(source) => serde_json::from_str(&source).map_err(|error| {
                CliError::Input(format!("invalid session file {}: {error}", path.display()))
            }),
            // Login creates this opt-in file after receiving the cookies. Let a
            // first login name a not-yet-existing path rather than requiring a
            // caller to create an empty file first.
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(CliError::Input(format!(
                "cannot read session file {}: {error}",
                path.display()
            ))),
        })
        .transpose()
        .map(|session| session.flatten())
    }

    pub(crate) fn save(path: &Path, session: String, csrf: String) -> Result<(), CliError> {
        let contents =
            serde_json::to_vec(&Self { session, csrf }).map_err(|_| CliError::InvalidResponse)?;
        let mut temporary = temporary_file(path, "session file")?;
        std::io::Write::write_all(temporary.as_file_mut(), &contents).map_err(|error| {
            CliError::Input(format!(
                "cannot write session file {}: {error}",
                path.display()
            ))
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o600)).map_err(
                |error| {
                    CliError::Input(format!(
                        "cannot secure session file {}: {error}",
                        path.display()
                    ))
                },
            )?;
        }
        temporary.as_file_mut().sync_all().map_err(|error| {
            CliError::Input(format!(
                "cannot sync session file {}: {error}",
                path.display()
            ))
        })?;
        persist_temporary_file(temporary, path, "session file")
    }
}

pub(crate) async fn save_session_response(
    response: reqwest::Response,
    session_file: Option<&PathBuf>,
) -> Result<String, CliError> {
    let cookies = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .filter_map(|header| header.split_once(';').map(|(cookie, _)| cookie))
        .filter_map(|cookie| cookie.split_once('='))
        .fold((None, None), |(session, csrf), (name, value)| match name {
            SESSION_COOKIE => (Some(value.to_owned()), csrf),
            CSRF_COOKIE => (session, Some(value.to_owned())),
            _ => (session, csrf),
        });
    let output = raw_response(response).await?;
    if let Some(path) = session_file {
        let (Some(session), Some(csrf)) = cookies else {
            return Err(CliError::InvalidResponse);
        };
        SessionFile::save(path, session, csrf)?;
    }
    Ok(output)
}
