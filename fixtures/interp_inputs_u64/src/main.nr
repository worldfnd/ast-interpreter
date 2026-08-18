// Fixture for Prover.toml input bridging. Computes the same u64 under either build field.
fn main(x: u64) -> pub u64 {
    let big: u64 = 18446744069414584322; // p + 1, a legal u64
    let prod = x * 2;
    prod + big
}
