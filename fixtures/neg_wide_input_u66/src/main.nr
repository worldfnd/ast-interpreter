// 2^65 is a legal u66 but exceeds the Goldilocks modulus: bn254 takes it as an input, while
// Goldilocks refuses a u66 entry point, since its values can reach the modulus.
fn main(x: u66) -> pub u66 {
    x
}
