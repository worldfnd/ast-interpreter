// Signed i32 inputs: 32-bit two's complement fits every supported field, so negative values
// decode identically under bn254 and Goldilocks. The body exercises signed negation, division
// that truncates toward zero, a remainder carrying the dividend's sign, and signed comparison.
fn main(a: i32, b: i32) -> pub i32 {
    let negated = -a;
    let quotient = a / b;
    let remainder = a % b;
    let larger = if a > b { a } else { b };
    negated + quotient * 10 + remainder * 100 + larger
}
