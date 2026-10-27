// An i64 return. A return is read as bytes, so every field takes it, though the pattern of a
// negative i64 can reach the Goldilocks modulus.
fn main(x: u8) -> pub i64 {
    -(x as i64)
}
