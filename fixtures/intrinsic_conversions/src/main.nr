// String <-> bytes conversions and array -> slice, all field-independent.
fn main() {
    let bytes: [u8; 2] = "hi".as_bytes();
    assert(bytes[0] == 104);
    assert(bytes[1] == 105);

    let raw: [u8; 2] = [104, 105];
    let s = raw.as_str_unchecked();
    let round: [u8; 2] = s.as_bytes();
    assert(round[0] == 104);
    assert(round[1] == 105);

    let v = [1, 2, 3].as_vector();
    assert(v.len() == 3);
}
