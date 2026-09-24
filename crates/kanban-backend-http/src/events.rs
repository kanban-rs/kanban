use kanban_api::ChangeEventFrame;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct SseParser {
    buf: Vec<u8>,
    data: String,
}

impl SseParser {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<ChangeEventFrame> {
        self.buf.extend_from_slice(chunk);
        let mut frames = Vec::new();

        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1]);
            let line = line.strip_suffix('\r').unwrap_or(&line).to_string();

            if line.is_empty() {
                if !self.data.is_empty() {
                    let data = std::mem::take(&mut self.data);
                    match serde_json::from_str::<ChangeEventFrame>(&data) {
                        Ok(frame) => frames.push(frame),
                        Err(e) => {
                            tracing::warn!("dropping malformed SSE change frame: {e}");
                        }
                    }
                }
            } else if line.starts_with(':') {
            } else if let Some(rest) = line.strip_prefix("data:") {
                let rest = rest.strip_prefix(' ').unwrap_or(rest);
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(rest);
            }
        }

        frames
    }
}

pub(crate) fn next_backoff(cur: Duration) -> Duration {
    (cur * 2).min(Duration::from_secs(30))
}

#[cfg(test)]
mod tests {
    use kanban_api::{ChangeEventFrame, InvalidationDto};
    use kanban_core::ClientId;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn test_sse_parser_parses_single_data_line_into_frame() {
        let frame = ChangeEventFrame::now(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ClientId::from(Uuid::new_v4()),
        )
        .with_invalidation(InvalidationDto::All);
        let json = serde_json::to_string(&frame).unwrap();
        let wire = format!("data: {json}\n\n");

        let mut parser = super::SseParser::default();
        let frames = parser.push(wire.as_bytes());

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].issued_by, frame.issued_by);
        assert_eq!(frames[0].invalidation, frame.invalidation);
    }

    #[test]
    fn test_sse_parser_ignores_keep_alive_comment_lines() {
        let mut parser = super::SseParser::default();
        let frames = parser.push(b":\n\n:\n\n: keep-alive\n\n");
        assert!(frames.is_empty());

        let frame = ChangeEventFrame::now(Uuid::new_v4(), Uuid::new_v4(), ClientId::nil());
        let json = serde_json::to_string(&frame).unwrap();
        let wire = format!("data: {json}\n\n");
        let frames = parser.push(wire.as_bytes());
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_sse_parser_reassembles_frame_split_across_chunks() {
        let frame = ChangeEventFrame::now(Uuid::new_v4(), Uuid::new_v4(), ClientId::nil());
        let json = serde_json::to_string(&frame).unwrap();
        let wire = format!("data: {json}\n\n");
        let bytes = wire.as_bytes();
        let mid = bytes.len() / 2;

        let mut parser = super::SseParser::default();
        let first = parser.push(&bytes[..mid]);
        assert!(first.is_empty());

        let second = parser.push(&bytes[mid..]);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].writer_instance_id, frame.writer_instance_id);
    }

    #[test]
    fn test_sse_parser_skips_malformed_data_line_and_continues() {
        let frame = ChangeEventFrame::now(Uuid::new_v4(), Uuid::new_v4(), ClientId::nil());
        let json = serde_json::to_string(&frame).unwrap();
        let wire = format!("data: {{not json\n\ndata: {json}\n\n");

        let mut parser = super::SseParser::default();
        let frames = parser.push(wire.as_bytes());

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].writer_instance_id, frame.writer_instance_id);
    }

    #[test]
    fn test_sse_parser_accepts_crlf_line_endings() {
        let frame = ChangeEventFrame::now(Uuid::new_v4(), Uuid::new_v4(), ClientId::nil());
        let json = serde_json::to_string(&frame).unwrap();
        let wire = format!("data: {json}\r\n\r\n");

        let mut parser = super::SseParser::default();
        let frames = parser.push(wire.as_bytes());

        assert_eq!(frames.len(), 1, "a CRLF-delimited frame must parse");
        assert_eq!(frames[0].writer_instance_id, frame.writer_instance_id);
    }

    #[test]
    fn test_sse_parser_joins_consecutive_data_lines_with_a_newline() {
        let frame = ChangeEventFrame::now(Uuid::new_v4(), Uuid::new_v4(), ClientId::nil());
        let json = serde_json::to_string_pretty(&frame).unwrap();
        let mut wire = String::new();
        for line in json.lines() {
            wire.push_str("data: ");
            wire.push_str(line);
            wire.push('\n');
        }

        let mut parser = super::SseParser::default();
        let pending = parser.push(wire.as_bytes());

        assert!(pending.is_empty(), "no blank line yet, so no frame yet");
        assert_eq!(
            parser.data, json,
            "consecutive data lines must be rejoined with a newline, byte for byte"
        );

        let frames = parser.push(b"\n");
        assert_eq!(frames.len(), 1, "multi-line data must parse as one payload");
        assert_eq!(frames[0].writer_instance_id, frame.writer_instance_id);
    }

    #[test]
    fn test_next_backoff_doubles_and_caps_at_thirty_seconds() {
        assert_eq!(
            super::next_backoff(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            super::next_backoff(Duration::from_secs(2)),
            Duration::from_secs(4)
        );
        assert_eq!(
            super::next_backoff(Duration::from_secs(16)),
            Duration::from_secs(30)
        );
        assert_eq!(
            super::next_backoff(Duration::from_secs(30)),
            Duration::from_secs(30)
        );
    }
}
