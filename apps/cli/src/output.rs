//! Standard output that ends quietly once its reader goes away.

use std::io::Write;

use crate::error::CliError;

/// Whether standard output still has a reader.
#[derive(Debug, PartialEq)]
pub(crate) enum Output {
    Open,
    /// The reader went away, as `| head -1` does after one line; the
    /// command has printed all anyone will read and ends successfully.
    Closed,
}

pub(crate) fn print_line(line: &str) -> Result<Output, CliError> {
    write_line(&mut std::io::stdout().lock(), line)
}

pub(crate) fn write_line(out: &mut impl Write, line: &str) -> Result<Output, CliError> {
    match writeln!(out, "{line}").and_then(|()| out.flush()) {
        Ok(()) => Ok(Output::Open),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(Output::Closed),
        Err(error) => Err(CliError::Transport(error.to_string())),
    }
}
