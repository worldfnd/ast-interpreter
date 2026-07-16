// Fixture for the tolerated-error file invariant: `wrapping_sub` lives in the stdlib's ops/arith.nr,
// whose `wrapping_sub_hlp` carries a 2^128 Field constant that fails to elaborate under Goldilocks.
// A program reaching such a file must be rejected loudly. Under bn254 the file is clean; this returns 2.
use std::ops::WrappingSub;

fn main(x: u64) -> pub u64 {
    x.wrapping_sub(1)
}
