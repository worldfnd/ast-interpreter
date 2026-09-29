// A string is its bytes: one that is not UTF-8 round-trips through `as_str_unchecked` and
// `as_bytes` at run time, through a global and through a comptime-folded literal.
global BROKEN: [u8; 3] = [0x41, 0xFF, 0x42];
global S: str<3> = BROKEN.as_str_unchecked();

fn main() -> pub [u8; 3] {
    let bytes: [u8; 3] = [0x41, 0xFF, 0x42];
    let s: str<3> = bytes.as_str_unchecked();
    let back: [u8; 3] = s.as_bytes();
    assert(back == bytes);
    assert(S.as_bytes() == bytes);
    assert(S == s);
    assert(s != "AxB");
    let folded = comptime {
        let raw = [102, 111, 128, 111_u8];
        raw.as_str_unchecked()
    };
    assert(folded.as_bytes() == [102, 111, 128, 111]);
    back
}
