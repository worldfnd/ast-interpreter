// The recorded i64 return parses under Goldilocks but its type is not representable there, so
// the return check does not run; the program itself runs under both fields.
fn main(x: u8) -> pub i64 {
    x as i64
}
