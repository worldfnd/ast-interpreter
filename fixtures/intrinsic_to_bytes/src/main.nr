// Field decomposition with a small value (258 = 0x0102) and small N, so bn254 and Goldilocks agree:
// the modulus-overflow guard never trips (N is well under both fields' byte/bit sizes). Drives
// `to_radix`, `static_assert`, `is_unconstrained`, and the const-folded `modulus_*` builtins.
fn main() {
    let x: Field = 258;

    let le: [u8; 4] = x.to_le_bytes();
    assert(le[0] == 2);
    assert(le[1] == 1);
    assert(le[2] == 0);
    assert(le[3] == 0);

    let be: [u8; 4] = x.to_be_bytes();
    assert(be[0] == 0);
    assert(be[1] == 0);
    assert(be[2] == 1);
    assert(be[3] == 2);

    // 258 = 0b1_0000_0010: bit 1 and bit 8 set.
    let bits: [bool; 10] = x.to_le_bits();
    assert(bits[0] == false);
    assert(bits[1] == true);
    assert(bits[8] == true);
    assert(bits[9] == false);
}
