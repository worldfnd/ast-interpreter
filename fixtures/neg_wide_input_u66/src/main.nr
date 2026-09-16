// 2^65 is a legal u66 but exceeds the Goldilocks modulus, so only bn254 can take it as an input.
fn main(x: u66) -> pub u66 {
    x
}
