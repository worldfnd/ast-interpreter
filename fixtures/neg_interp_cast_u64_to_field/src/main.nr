// `u64` can exceed the Goldilocks modulus, so `x as Field` is a compile error there
// (`IntegerTypeExceedsField`); under bn254 the cast is the identity on the value.
fn main(x: u64) -> pub Field {
    x as Field
}
