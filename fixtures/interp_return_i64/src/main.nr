// An i64 return: Goldilocks refuses it, since the pattern of a negative i64 can reach its modulus.
fn main(x: u8) -> pub i64 {
    -(x as i64)
}
