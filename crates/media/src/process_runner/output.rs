//! Bounded pipe decoding, streaming redaction and line framing.

use super::Stream;
use std::ffi::OsString;

const LINE_LIMIT: usize = 65_536;
const TAIL_LIMIT: usize = 8192;

pub(super) struct Secrets(Vec<String>);

impl Secrets {
    pub(super) fn from_args(args: &[OsString]) -> Self {
        let mut values = Vec::new();
        for pair in args.windows(2) {
            let name = pair[0].to_string_lossy().to_ascii_lowercase();
            if ["key", "token", "password", "secret", "authorization"]
                .iter()
                .any(|word| name.contains(word))
            {
                let value = pair[1].to_string_lossy().into_owned();
                if !value.is_empty() {
                    values.push(value);
                }
            }
        }
        Self(values)
    }

    pub(super) fn redact(&self, text: &str) -> String {
        let mut pending = text.to_owned();
        self.drain(&mut pending, true)
    }

    // Keep only a suffix that might become a secret when more text arrives.
    // Memory is bounded by one input chunk plus the longest secret argument.
    fn drain(&self, pending: &mut String, eof: bool) -> String {
        let mut clean = String::new();
        let mut consumed = 0;
        while consumed < pending.len() {
            let rest = &pending[consumed..];
            if !eof
                && self
                    .0
                    .iter()
                    .any(|value| value.len() > rest.len() && value.starts_with(rest))
            {
                break;
            }
            if let Some(length) = self
                .0
                .iter()
                .filter(|value| rest.starts_with(value.as_str()))
                .map(String::len)
                .max()
            {
                clean.push_str("***");
                consumed += length;
            } else if let Some(ch) = rest.chars().next() {
                clean.push(ch);
                consumed += ch.len_utf8();
            }
        }
        pending.drain(..consumed);
        clean
    }
}

pub(super) struct Output {
    stream: Stream,
    undecoded: Vec<u8>,
    unredacted: String,
    line: String,
    pub(super) tail: String,
}

impl Output {
    pub(super) fn new(stream: Stream) -> Self {
        Self {
            stream,
            undecoded: Vec::new(),
            unredacted: String::new(),
            line: String::new(),
            tail: String::new(),
        }
    }

    pub(super) fn feed(
        &mut self,
        bytes: &[u8],
        eof: bool,
        secrets: &Secrets,
        on_line: &mut impl FnMut(Stream, &str),
    ) {
        self.undecoded.extend_from_slice(bytes);
        let mut consumed = 0;
        while consumed < self.undecoded.len() {
            let rest = &self.undecoded[consumed..];
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    self.unredacted.push_str(text);
                    consumed = self.undecoded.len();
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    self.unredacted
                        .push_str(&String::from_utf8_lossy(&rest[..valid]));
                    consumed += valid;
                    if let Some(length) = error.error_len() {
                        self.unredacted.push('\u{fffd}');
                        consumed += length;
                    } else if eof {
                        self.unredacted.push('\u{fffd}');
                        consumed = self.undecoded.len();
                    } else {
                        break;
                    }
                }
            }
        }
        self.undecoded.drain(..consumed);
        let text = secrets.drain(&mut self.unredacted, eof);
        if self.stream == Stream::Stderr {
            self.tail.push_str(&text);
            if self.tail.len() > TAIL_LIMIT {
                let mut start = self.tail.len() - TAIL_LIMIT;
                while !self.tail.is_char_boundary(start) {
                    start += 1;
                }
                self.tail.drain(..start);
            }
        }
        for ch in text.chars() {
            if ch == '\n' {
                on_line(
                    self.stream,
                    self.line.strip_suffix('\r').unwrap_or(&self.line),
                );
                self.line.clear();
            } else {
                if self.line.len() + ch.len_utf8() > LINE_LIMIT {
                    on_line(self.stream, &self.line);
                    self.line.clear();
                }
                self.line.push(ch);
            }
        }
        if eof && !self.line.is_empty() {
            on_line(self.stream, &self.line);
            self.line.clear();
        }
    }
}
