// `assert_max_bit_size` lowers to the `apply_range_constraint` builtin. A bit-size bound below both
// field moduli (48 < 64 Goldilocks < 254 bn254) is field-independent, so the check holds identically
// on both fields. `x = 3` fits in 48 bits, so `main` interprets to a clean `Unit`.
fn main(x: Field) {
    x.assert_max_bit_size::<48>();
}
