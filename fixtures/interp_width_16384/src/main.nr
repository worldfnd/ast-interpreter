use std::cmp::Ordering;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

// An operator on a primitive never consults its trait impl; a bound does.
fn arith<T>(x: T, y: T) -> T
where
    T: Add + Sub + Mul + Div + Rem,
{
    ((x + y) - y) * y / y % (y + y)
}

fn bits<T>(x: T, y: T) -> T
where
    T: BitAnd + BitOr + BitXor + Not,
{
    !((x & y) | (x ^ y))
}

fn shifts<T>(x: T, by: T) -> T
where
    T: Shl + Shr,
{
    (x << by) >> by
}

fn order<T>(x: T, y: T) -> bool
where
    T: Ord + Eq,
{
    (x.cmp(y) == Ordering::less()) & (x != y)
}

fn negate<T>(x: T) -> T
where
    T: Neg,
{
    -x
}

fn main(a: u16384, b: u16384) -> pub u16384 {
    let zero: u16384 = 0;
    let max: u16384 = !zero;
    let top: u16384 = 1 << 16383;
    assert(max - top == top - 1);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert(a + b == 8);
    assert(b - a == 2);
    assert(max % b == 0);
    assert(max % a == 0);
    assert(max / b * b == max);
    assert(max / b == (top - 1) / b * 2 + 1);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!a == max - 3);
    assert((top | a) == top + 3);
    assert((max & top) == top);
    assert((max ^ top) == top - 1);
    assert(a << 16381 == (top >> 1) + (top >> 2));
    assert(a << 16383 == top);
    assert(top >> 16383 == 1);
    assert(max >> 1 == top - 1);
    assert(max >> 16383 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 18446744073709551615);
    assert(max as u128 == 340282366920938463463374607431768211455);
    assert(top as u128 == 0);
    assert(max as i16384 == -1);
    assert((a as u8) as u16384 == a);
    assert((12345 as Field) as u16384 == 12345);
    assert((a as u128) as u16384 == a);
    assert(a < b);
    assert(a <= b);
    assert(a != b);
    assert(top > max - top);
    let sa: i16384 = a as i16384;
    let neg: i16384 = 0 - sa;
    let smin: i16384 = i16384::min_value();
    let smax: i16384 = i16384::max_value();
    assert(smin == top as i16384);
    assert(smax == (top - 1) as i16384);
    assert(neg + sa == 0);
    assert(neg * sa == -9);
    assert(smin + 1 == 0 - smax);
    assert(smax + smin == -1);
    assert(smax - sa + sa == smax);
    assert(smin % sa == -2);
    assert(smin / sa * sa + smin % sa == smin);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 16383 == -1);
    assert(smin < smax);
    assert(neg as u16384 == max - 2);
    assert(smin as u16384 == top);
    assert(neg as u8 == 253);
    assert((neg as i8) as i16384 == neg);
    assert(smin as i64 == 0);
    assert(smin as u128 == 0);
    assert(u16384::max_value() == max);
    assert(u16384::min_value() == 0);
    assert(u16384::bits() == 16384);
    assert(i16384::bits() == 16384);
    let zero_u: u16384 = std::default::Default::default();
    let zero_i: i16384 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == max - 7);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 16383)
}
