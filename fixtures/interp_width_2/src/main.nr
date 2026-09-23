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

fn check(a: u2, b: u2) -> u2 {
    let top: u2 = 1 << 1;
    let max: u2 = 3;
    assert(a + b == 3);
    assert(b - a == 1);
    assert(max / b == 1);
    assert(max % b == 1);
    assert(b / a == 2);
    assert(b % a == 0);
    assert(max - top == 1);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 0);
    assert((a | b) == 3);
    assert((a ^ b) == 3);
    assert(!a == 2);
    assert(!b == 1);
    assert((top | a) == 3);
    assert((max & top) == top);
    assert((max ^ top) == 1);
    assert(a << 1 == 2);
    assert(b << 1 == 0);
    assert(top >> 1 == 1);
    assert(max >> 1 == 1);
    assert((b >> 1) << 1 == b);
    assert(max as u8 == 3);
    assert(max as i2 == -1);
    assert(b as i2 == -2);
    assert((a as u8) as u2 == a);
    assert((255 as u8) as u2 == 3);
    assert((6 as u8) as u2 == 2);
    assert((5 as Field) as u2 == 1);
    assert((max as Field) == 3);
    assert((max as u64 + 1) as u2 == 0);
    assert(a < b);
    assert(a <= b);
    assert(a != b);
    assert(top > max - top);
    let sa: i2 = a as i2;
    let neg: i2 = 0 - sa;
    let smin: i2 = -2;
    let smax: i2 = 1;
    assert(neg + sa == 0);
    assert(neg * sa == -1);
    assert(smin + 1 == -1);
    assert(smax + smin == -1);
    assert(smax - sa == 0);
    assert(smin / sa == -2);
    assert(neg % sa == 0);
    assert(smin % sa == 0);
    assert(neg >> 1 == -1);
    assert(neg << 1 == -2);
    assert(smin >> 1 == -1);
    assert(smax >> 1 == 0);
    assert(smin < smax);
    assert(neg as u2 == 3);
    assert(smin as u2 == top);
    assert(neg as u8 == 255);
    assert((neg as i8) as i2 == neg);
    assert(smin as i64 == -2);
    assert(smin as u8 == 254);
    assert((smin as i8) as i2 == smin);
    assert(u2::max_value() == max);
    assert(u2::min_value() == 0);
    assert(u2::bits() == 2);
    assert(i2::max_value() == smax);
    assert(i2::min_value() == smin);
    assert(i2::bits() == 2);
    let zero_u: u2 = std::default::Default::default();
    let zero_i: i2 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, a) == 1);
    assert(arith(sa, neg) == 1);
    assert(bits(a, b) == 0);
    assert(shifts(a, 1) == a);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 1)
}

fn main(a: u8, b: u8) -> pub u8 {
    check(a as u2, b as u2) as u8
}
