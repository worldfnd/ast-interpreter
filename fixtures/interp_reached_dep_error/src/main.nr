// u32 wrapping subtraction reaches ops/arith.nr, whose 2^128 Field constant fails under
// Goldilocks. u64 wrapping subtraction is gated out before reaching that file.
use std::ops::WrappingSub;

fn main(x: u32) -> pub u32 {
    x.wrapping_sub(1)
}
