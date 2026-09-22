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

fn main(a: u33, b: u33) -> pub u33 {
    let top: u33 = 1 << 32;
    let max: u33 = 8589934591;
    assert(a + b == 8);
    assert(b - a == 2);
    assert(max / b == 1717986918);
    assert(max % b == 1);
    assert(max - top == 4294967295);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!a == 8589934588);
    assert((top | a) == 4294967299);
    assert((max & top) == top);
    assert((max ^ top) == 4294967295);
    assert(a << 30 == 3221225472);
    assert(a << 32 == 4294967296);
    assert(top >> 32 == 1);
    assert(max >> 1 == 4294967295);
    assert(max >> 32 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 8589934591);
    assert(max as i33 == -1);
    assert((a as u8) as u33 == a);
    assert((12345 as Field) as u33 == 12345);
    assert((max as Field) == 8589934591);
    assert((max as u64 + 1) as u33 == 0);
    assert(a < b);
    assert(a <= b);
    assert(a != b);
    assert(top > max - top);
    let sa: i33 = a as i33;
    let neg: i33 = 0 - sa;
    let smin: i33 = -4294967296;
    let smax: i33 = 4294967295;
    assert(neg + sa == 0);
    assert(neg * sa == -9);
    assert(smin + 1 == -4294967295);
    assert(smax + smin == -1);
    assert(smax - sa == 4294967292);
    assert(smin / sa == -1431655765);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 32 == -1);
    assert(smin < smax);
    assert(neg as u33 == 8589934589);
    assert(smin as u33 == top);
    assert(neg as u8 == 253);
    assert((neg as i8) as i33 == neg);
    assert(smin as i64 == -4294967296);
    assert(u33::max_value() == max);
    assert(u33::min_value() == 0);
    assert(u33::bits() == 33);
    assert(i33::max_value() == smax);
    assert(i33::min_value() == smin);
    assert(i33::bits() == 33);
    let zero_u: u33 = std::default::Default::default();
    let zero_i: i33 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 8589934584);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 32)
}
