use super::frame::Frame;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeResult {
    Complete(Frame, usize),
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespError {
    message: String,
}

impl RespError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RespError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for RespError {}

pub fn decode(input: &[u8]) -> Result<DecodeResult, RespError> {
    decode_at(input, 0)
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let mut output = Vec::new();
    encode_into(frame, &mut output);
    output
}

pub fn frame_to_parts(frame: Frame) -> Result<Option<Vec<String>>, RespError> {
    match frame {
        Frame::Array(items) => {
            if items.is_empty() {
                return Ok(None);
            }

            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Frame::Bulk(value) | Frame::Simple(value) => parts.push(value),
                    _ => return Err(RespError::new("command arrays must contain strings")),
                }
            }

            Ok(Some(parts))
        }
        Frame::Bulk(value) | Frame::Simple(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            Ok(Some(
                trimmed
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect(),
            ))
        }
        _ => Err(RespError::new("expected command array")),
    }
}

fn decode_at(input: &[u8], start: usize) -> Result<DecodeResult, RespError> {
    if start >= input.len() {
        return Ok(DecodeResult::Incomplete);
    }

    match input[start] {
        b'+' => decode_simple(input, start, Frame::Simple),
        b'-' => decode_simple(input, start, Frame::Error),
        b':' => decode_integer(input, start),
        b'$' => decode_bulk(input, start),
        b'*' => decode_array(input, start),
        _ => Err(RespError::new("invalid frame type")),
    }
}

fn decode_simple(
    input: &[u8],
    start: usize,
    frame: impl FnOnce(String) -> Frame,
) -> Result<DecodeResult, RespError> {
    let Some((line, consumed)) = read_line(input, start + 1) else {
        return Ok(DecodeResult::Incomplete);
    };
    let value = bytes_to_string(line)?;

    Ok(DecodeResult::Complete(frame(value), consumed))
}

fn decode_integer(input: &[u8], start: usize) -> Result<DecodeResult, RespError> {
    let Some((line, consumed)) = read_line(input, start + 1) else {
        return Ok(DecodeResult::Incomplete);
    };
    let text = bytes_to_string(line)?;
    let value = text
        .parse::<i64>()
        .map_err(|_| RespError::new("invalid integer"))?;

    Ok(DecodeResult::Complete(Frame::Integer(value), consumed))
}

fn decode_bulk(input: &[u8], start: usize) -> Result<DecodeResult, RespError> {
    let Some((line, line_consumed)) = read_line(input, start + 1) else {
        return Ok(DecodeResult::Incomplete);
    };
    let length = parse_length(line)?;

    if length < 0 {
        return Ok(DecodeResult::Complete(Frame::Null, line_consumed));
    }

    let length = length as usize;
    let content_start = line_consumed;
    let content_end = content_start + length;

    if input.len() < content_end + 1 {
        return Ok(DecodeResult::Incomplete);
    }

    let Some(terminator_len) = line_terminator_len(input, content_end) else {
        if input[content_end] == b'\r' && input.len() == content_end + 1 {
            return Ok(DecodeResult::Incomplete);
        }
        return Err(RespError::new("bulk string missing terminator"));
    };

    let value = bytes_to_string(&input[content_start..content_end])?;
    Ok(DecodeResult::Complete(
        Frame::Bulk(value),
        content_end + terminator_len,
    ))
}

fn decode_array(input: &[u8], start: usize) -> Result<DecodeResult, RespError> {
    let Some((line, mut consumed)) = read_line(input, start + 1) else {
        return Ok(DecodeResult::Incomplete);
    };
    let length = parse_length(line)?;

    if length < 0 {
        return Ok(DecodeResult::Complete(Frame::Null, consumed));
    }

    let mut items = Vec::with_capacity(length as usize);
    for _ in 0..length {
        match decode_at(input, consumed)? {
            DecodeResult::Complete(frame, frame_consumed) => {
                consumed = frame_consumed;
                items.push(frame);
            }
            DecodeResult::Incomplete => return Ok(DecodeResult::Incomplete),
        }
    }

    Ok(DecodeResult::Complete(Frame::Array(items), consumed))
}

fn read_line(input: &[u8], start: usize) -> Option<(&[u8], usize)> {
    for index in start..input.len() {
        if input[index] == b'\n' {
            let line_end = if index > start && input[index - 1] == b'\r' {
                index - 1
            } else {
                index
            };
            return Some((&input[start..line_end], index + 1));
        }
    }

    None
}

fn line_terminator_len(input: &[u8], start: usize) -> Option<usize> {
    match input.get(start..) {
        Some(bytes) if bytes.starts_with(b"\r\n") => Some(2),
        Some(bytes) if bytes.starts_with(b"\n") => Some(1),
        _ => None,
    }
}

fn parse_length(bytes: &[u8]) -> Result<isize, RespError> {
    let text = bytes_to_string(bytes)?;
    text.parse::<isize>()
        .map_err(|_| RespError::new("invalid length"))
}

fn bytes_to_string(bytes: &[u8]) -> Result<String, RespError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| RespError::new("invalid UTF-8 string"))
}

fn encode_into(frame: &Frame, output: &mut Vec<u8>) {
    match frame {
        Frame::Simple(value) => {
            output.extend_from_slice(b"+");
            output.extend_from_slice(value.as_bytes());
            output.extend_from_slice(b"\r\n");
        }
        Frame::Bulk(value) => {
            output.extend_from_slice(format!("${}\r\n", value.len()).as_bytes());
            output.extend_from_slice(value.as_bytes());
            output.extend_from_slice(b"\r\n");
        }
        Frame::Integer(value) => {
            output.extend_from_slice(format!(":{value}\r\n").as_bytes());
        }
        Frame::Array(items) => {
            output.extend_from_slice(format!("*{}\r\n", items.len()).as_bytes());
            for item in items {
                encode_into(item, output);
            }
        }
        Frame::Error(value) => {
            output.extend_from_slice(b"-");
            output.extend_from_slice(value.as_bytes());
            output.extend_from_slice(b"\r\n");
        }
        Frame::Null => output.extend_from_slice(b"$-1\r\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeResult, decode, encode, frame_to_parts};
    use crate::protocol::frame::Frame;

    #[test]
    fn decodes_array_command() {
        let input = b"*2\r\n$3\r\nGET\r\n$4\r\nname\r\n";

        assert_eq!(
            decode(input),
            Ok(DecodeResult::Complete(
                Frame::Array(vec![
                    Frame::Bulk("GET".to_string()),
                    Frame::Bulk("name".to_string())
                ]),
                input.len()
            ))
        );
    }

    #[test]
    fn reports_incomplete_frame() {
        assert_eq!(decode(b"*2\r\n$3\r\nGET\r\n"), Ok(DecodeResult::Incomplete));
    }

    #[test]
    fn supports_lf_only_manual_input() {
        let input = b"*1\n$4\nPING\n";

        assert_eq!(
            decode(input),
            Ok(DecodeResult::Complete(
                Frame::Array(vec![Frame::Bulk("PING".to_string())]),
                input.len()
            ))
        );
    }

    #[test]
    fn encodes_bulk_and_null() {
        assert_eq!(
            encode(&Frame::Bulk("Water".to_string())),
            b"$5\r\nWater\r\n"
        );
        assert_eq!(encode(&Frame::Null), b"$-1\r\n");
    }

    #[test]
    fn encodes_and_decodes_integer() {
        assert_eq!(encode(&Frame::Integer(1)), b":1\r\n");
        assert_eq!(
            decode(b":42\r\n"),
            Ok(DecodeResult::Complete(Frame::Integer(42), 5))
        );
    }

    #[test]
    fn converts_array_to_command_parts() {
        let frame = Frame::Array(vec![
            Frame::Bulk("SET".to_string()),
            Frame::Bulk("name".to_string()),
            Frame::Bulk("Water River".to_string()),
        ]);

        assert_eq!(
            frame_to_parts(frame),
            Ok(Some(vec![
                "SET".to_string(),
                "name".to_string(),
                "Water River".to_string()
            ]))
        );
    }
}
