// Signed i64 input: its 2^64 range exceeds the Goldilocks modulus, so the two's-complement ABI
// encoding is not injective there. The input bridge must reject it rather than decode a wrong
// positive value; on bn254 the same input decodes correctly.
fn main(x: i64) -> pub i64 {
    x
}
