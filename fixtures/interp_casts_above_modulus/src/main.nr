// Integer casts preserve values above the Goldilocks modulus at runtime and comptime.
fn main() {
    let x: u64 = 18446744073709551615;
    assert((x as u128) == 18446744073709551615);
    assert((x as u8) == 255);
    assert((x as i64) == -1);
    let m: i8 = -1;
    assert((m as u8) == 255);
    assert(((m as u8) as Field) == 255);

    comptime {
        let x: u64 = 18446744073709551615;
        assert((x as u128) == 18446744073709551615);
        assert((x as u8) == 255);
        assert((x as i64) == -1);
        let m: i8 = -1;
        assert((m as u8) == 255);
        assert(((m as u8) as Field) == 255);
    }
}
