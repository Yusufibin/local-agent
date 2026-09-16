//! Strict JSONL framer: split on LF only, strip a trailing CR, never on U+2028/U+2029.

pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Record(Vec<u8>),
    Oversize,
}

#[derive(Debug, Default)]
pub struct JsonlFramer {
    buf: Vec<u8>,
    max: usize,
    dropping_oversize: bool,
    oversize_signaled: bool,
}

impl JsonlFramer {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            max: MAX_LINE_BYTES,
            dropping_oversize: false,
            oversize_signaled: false,
        }
    }

    pub fn with_max(max: usize) -> Self {
        Self {
            buf: Vec::new(),
            max,
            dropping_oversize: false,
            oversize_signaled: false,
        }
    }

    pub fn leftover(&self) -> &[u8] {
        &self.buf
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Frame> {
        let mut out = Vec::new();
        for &b in bytes {
            if self.dropping_oversize {
                if b == b'\n' {
                    self.dropping_oversize = false;
                    self.oversize_signaled = false;
                    self.buf.clear();
                }
                continue;
            }
            if b == b'\n' {
                let mut line = std::mem::take(&mut self.buf);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.len() > self.max {
                    out.push(Frame::Oversize);
                } else {
                    out.push(Frame::Record(line));
                }
            } else {
                self.buf.push(b);
                if self.buf.len() > self.max {
                    self.buf.clear();
                    self.dropping_oversize = true;
                    if !self.oversize_signaled {
                        self.oversize_signaled = true;
                        out.push(Frame::Oversize);
                    }
                }
            }
        }
        out
    }

    pub fn record_to_utf8(bytes: &[u8]) -> Option<String> {
        String::from_utf8(bytes.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_lf_only() {
        let mut f = JsonlFramer::new();
        let frames = f.push(b"{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(frames.len(), 2);
        match &frames[0] {
            Frame::Record(r) => assert_eq!(r, b"{\"a\":1}"),
            _ => panic!("expected record"),
        }
    }

    #[test]
    fn strips_trailing_cr_from_crlf() {
        let mut f = JsonlFramer::new();
        let frames = f.push(b"{\"a\":1}\r\n");
        match &frames[0] {
            Frame::Record(r) => assert_eq!(r, b"{\"a\":1}"),
            _ => panic!("expected record"),
        }
    }

    #[test]
    fn unicode_separators_do_not_split() {
        let mut f = JsonlFramer::new();
        // U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR inside a JSON string.
        let payload = "{\"x\":\"hello\u{2028}mid\u{2029}world\"}\n";
        let frames = f.push(payload.as_bytes());
        assert_eq!(frames.len(), 1);
        match &frames[0] {
            Frame::Record(r) => {
                let s = String::from_utf8(r.clone()).unwrap();
                assert!(s.contains('\u{2028}'));
                assert!(s.contains('\u{2029}'));
                assert!(!s.contains('\n'));
            }
            _ => panic!("expected a single record"),
        }
    }

    #[test]
    fn partial_last_line_stays_buffered() {
        let mut f = JsonlFramer::new();
        let frames = f.push(b"{\"a\":1}\n{\"b\":");
        assert_eq!(frames.len(), 1);
        assert_eq!(f.leftover(), b"{\"b\":");
    }

    #[test]
    fn oversized_line_is_dropped_and_signaled() {
        let mut f = JsonlFramer::with_max(8);
        let mut data = b"abcdefghij".to_vec(); // 10 > 8, no newline yet
        data.push(b'\n');
        data.extend_from_slice(b"ok\n");
        let frames = f.push(&data);
        assert!(frames.iter().any(|f| matches!(f, Frame::Oversize)));
        let records: Vec<_> = frames
            .iter()
            .filter_map(|f| match f {
                Frame::Record(r) => Some(r.as_slice()),
                _ => None,
            })
            .collect();
        assert_eq!(records, vec![&b"ok"[..]]);
    }
}
