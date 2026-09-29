// An i64 input: bn254 decodes it; Goldilocks refuses an i64 entry point, whose pattern for -1 is
// 2^64 - 1.
fn main(x: i64) -> pub i64 {
    x
}
