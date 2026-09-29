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

fn main(a: u10, b: u10) -> pub u10 {
    let top: u10 = 1 << 9;
    let max: u10 = 1023;
    assert(a + b == 8);
    assert(b - a == 2);
    assert(max / b == 204);
    assert(max % b == 3);
    assert(max - top == 511);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!a == 1020);
    assert((top | a) == 515);
    assert((max & top) == top);
    assert((max ^ top) == 511);
    assert(a << 7 == 384);
    assert(a << 9 == 512);
    assert(top >> 9 == 1);
    assert(max >> 1 == 511);
    assert(max >> 9 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as i10 == -1);
    assert((a as u8) as u10 == a);
    assert((12345 as Field) as u10 == 57);
    assert((max as Field) == 1023);
    assert((max as u64 + 1) as u10 == 0);
    assert(a < b);
    assert(a <= b);
    assert(a != b);
    assert(top > max - top);
    let sa: i10 = a as i10;
    let neg: i10 = 0 - sa;
    let smin: i10 = -512;
    let smax: i10 = 511;
    assert(neg + sa == 0);
    assert(neg * sa == -9);
    assert(smin + 1 == -511);
    assert(smax + smin == -1);
    assert(smax - sa == 508);
    assert(smin / sa == -170);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 9 == -1);
    assert(smin < smax);
    assert(neg as u10 == 1021);
    assert(smin as u10 == top);
    assert(neg as u8 == 253);
    assert((neg as i8) as i10 == neg);
    assert(smin as i64 == -512);
    assert(u10::max_value() == max);
    assert(u10::min_value() == 0);
    assert(u10::bits() == 10);
    assert(i10::max_value() == smax);
    assert(i10::min_value() == smin);
    assert(i10::bits() == 10);
    let zero_u: u10 = std::default::Default::default();
    let zero_i: i10 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 1016);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 9)
}
