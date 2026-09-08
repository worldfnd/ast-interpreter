// Signed i64 input: the ABI parser refuses `x = "-1"` under Goldilocks because its two's
// complement pattern, 2^64 - 1, exceeds the modulus; bn254 decodes it.
fn main(x: i64) -> pub i64 {
    x
}
