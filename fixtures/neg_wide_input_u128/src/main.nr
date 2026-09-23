// 2^65 is a legal u128 but exceeds the Goldilocks modulus: bn254 takes it as an input, while
// Goldilocks refuses a u128 entry point, since its values can reach the modulus.
fn main(x: u128) -> pub u128 {
    x
}
