// `u64` can exceed the Goldilocks modulus, so `x as Field` is a compile error there
// (`IntegerTypeExceedsField`); under bn254 the cast is the identity on the value. The `u64` is
// assembled from two halves, since an entry point takes only integers narrower than the field.
fn main(hi: u32, lo: u32) -> pub Field {
    let x: u64 = ((hi as u64) << 32) | (lo as u64);
    x as Field
}
