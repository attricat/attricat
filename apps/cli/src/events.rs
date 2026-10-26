//! Server-sent event framing for streamed agent runs.

/// One dispatched server-sent event.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ServerEvent {
    pub(crate) id: Option<String>,
    pub(crate) event: Option<String>,
    pub(crate) data: String,
}

/// Splits a byte stream into events. Comments and unknown fields are
/// ignored, as the EventSource specification requires.
#[derive(Default)]
pub(crate) struct EventParser {
    buffer: Vec<u8>,
    current: ServerEvent,
    has_data: bool,
}

impl EventParser {
    /// Adds received bytes and returns every event they complete.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Vec<ServerEvent> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                if self.has_data {
                    events.push(std::mem::take(&mut self.current));
                } else {
                    self.current = ServerEvent::default();
                }
                self.has_data = false;
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "id" => self.current.id = Some(value.to_owned()),
                "event" => self.current.event = Some(value.to_owned()),
                "data" => {
                    if self.has_data {
                        self.current.data.push('\n');
                    }
                    self.current.data.push_str(value);
                    self.has_data = true;
                }
                _ => {}
            }
        }
        events
    }
}
