use encoding_rs::GBK;
use std::fs;

fn main() {
    let lrc_content = "[00:00.00]汪苏泷 - 万有引力";
    let (bytes, _, _) = GBK.encode(lrc_content);
    fs::write("test_gbk.lrc", bytes).unwrap();

    let read_bytes = fs::read("test_gbk.lrc").unwrap();
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&read_bytes, true);
    let encoding = detector.guess(None, false);

    let (decoded, _, _) = encoding.decode(&read_bytes);
    println!("Detected encoding: {}", encoding.name());
    println!("Decoded content: {}", decoded);

    assert!(decoded.contains("汪苏泷"));
}
