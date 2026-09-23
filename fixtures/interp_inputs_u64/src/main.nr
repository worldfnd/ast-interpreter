// Fixture for Prover.toml input bridging of a u64, which only bn254 admits at an entry point.
fn main(x: u64) -> pub u64 {
    let big: u64 = 18446744069414584322; // p + 1, a legal u64
    let prod = x * 2;
    prod + big
}
