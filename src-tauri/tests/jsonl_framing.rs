use deskpi_lib::jsonl::{Frame, JsonlFramer, MAX_LINE_BYTES};

#[test]
fn splits_on_lf_accepts_crlf_keeps_unicode_separators_buffers_partial_drops_oversize() {
    let mut framer = JsonlFramer::new();

    let lf = framer.push(b"{\"a\":1}\n{\"b\":2}\n");
    assert_eq!(lf.len(), 2);
    assert!(matches!(&lf[0], Frame::Record(r) if r == b"{\"a\":1}"));
    assert!(matches!(&lf[1], Frame::Record(r) if r == b"{\"b\":2}"));

    let mut framer = JsonlFramer::new();
    let crlf = framer.push(b"{\"a\":1}\r\n");
    assert!(matches!(&crlf[0], Frame::Record(r) if r == b"{\"a\":1}"));

    let mut framer = JsonlFramer::new();
    let payload = "{\"x\":\"hello\u{2028}mid\u{2029}world\"}\n";
    let frames = framer.push(payload.as_bytes());
    assert_eq!(frames.len(), 1, "U+2028/U+2029 must not create a new record");
    match &frames[0] {
        Frame::Record(r) => {
            let s = String::from_utf8(r.clone()).unwrap();
            assert!(s.contains('\u{2028}'));
            assert!(s.contains('\u{2029}'));
        }
        Frame::Oversize => panic!("not oversize"),
    }

    let mut framer = JsonlFramer::new();
    let partial = framer.push(b"{\"ok\":true}\n{\"partial\":");
    assert_eq!(partial.len(), 1);
    assert_eq!(framer.leftover(), b"{\"partial\":");

    let mut framer = JsonlFramer::new();
    let mut huge = vec![b'x'; MAX_LINE_BYTES + 8];
    huge.push(b'\n');
    huge.extend_from_slice(b"{\"ok\":1}\n");
    let frames = framer.push(&huge);
    assert!(
        frames.iter().any(|f| matches!(f, Frame::Oversize)),
        "oversize signal must fire"
    );
    let records: Vec<_> = frames
        .into_iter()
        .filter_map(|f| match f {
            Frame::Record(r) => Some(r),
            Frame::Oversize => None,
        })
        .collect();
    assert_eq!(records, vec![b"{\"ok\":1}".to_vec()]);
}
