use std::ops::{WrappingAdd, WrappingMul, WrappingSub};

fn main(x: u64) -> pub u64 {
    let zero64: u64 = 0;
    let max64: u64 = 18446744073709551615;
    let zero128: u128 = 0;
    let max128: u128 = 340282366920938463463374607431768211455;
    assert(zero64.wrapping_sub(x) == max64);
    assert(zero128.wrapping_sub(x as u128) == max128);
    assert(max64.wrapping_add(x) == 0);
    assert(max64.wrapping_mul(max64) == 1);
    assert(max128.wrapping_mul(max128) == 1);
    assert(max128.wrapping_add(x as u128) == 0);
    assert(max64.wrapping_mul(2) == max64 - 1);
    assert(max128.wrapping_mul(2) == max128 - 1);
    assert((40 as u64).wrapping_add(2) == 42);
    assert((44 as u128).wrapping_sub(2) == 42);
    assert((6 as u64).wrapping_mul(7) == 42);
    assert((6 as u128).wrapping_mul(7) == 42);
    let max8: u8 = 255;
    assert(max8.wrapping_add(x as u8) == 0);
    assert((0 as u8).wrapping_sub(x as u8) == 255);
    assert(max8.wrapping_mul(max8) == 1);
    let max16: u16 = 65535;
    assert(max16.wrapping_add(x as u16) == 0);
    assert((0 as u16).wrapping_sub(x as u16) == 65535);
    assert(max16.wrapping_mul(max16) == 1);
    let max32: u32 = 4294967295;
    assert(max32.wrapping_add(x as u32) == 0);
    assert((0 as u32).wrapping_sub(x as u32) == 4294967295);
    assert(max32.wrapping_mul(max32) == 1);
    assert((3 as u32).wrapping_sub(x as u32) == 2);
    let min64: i64 = -9223372036854775807 - 1;
    let imax64: i64 = 9223372036854775807;
    let minus_one: i64 = -1;
    assert(min64.wrapping_sub(x as i64) == imax64);
    assert(imax64.wrapping_add(x as i64) == min64);
    assert(minus_one.wrapping_mul(minus_one) == 1);
    assert(minus_one.wrapping_add(x as i64) == 0);
    assert(imax64.wrapping_mul(2) == -2);
    max64.wrapping_add(x).wrapping_add(7)
}
