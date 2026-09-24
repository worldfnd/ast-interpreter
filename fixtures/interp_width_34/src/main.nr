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

fn check(a: u34, b: u34) -> u34 {
    let top: u34 = 1 << 33;
    let max: u34 = 17179869183;
    assert(top == 8589934592);
    assert(a + b == 8);
    assert(b - a == 2);
    assert(a * b == 15);
    assert(max / b == 3435973836);
    assert(max % b == 3);
    assert(max - top == 8589934591);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!max == 0);
    assert(!a == 17179869180);
    assert((top | a) == 8589934595);
    assert((max & top) == top);
    assert((max ^ top) == 8589934591);
    assert(a << 31 == 6442450944);
    assert(a << 33 == 8589934592);
    assert(top >> 33 == 1);
    assert(max >> 1 == 8589934591);
    assert(max >> 33 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 17179869183);
    assert(max as i34 == -1);
    assert(a as i34 == 3);
    assert((a as u8) as u34 == a);
    assert((12345 as Field) as u34 == 12345);
    assert((a as Field) == 3);
    assert((top as Field) == 8589934592);
    assert((max as Field) == 17179869183);
    assert((max as u64 + 1) as u34 == 0);
    assert(a < b);
    assert(a <= b);
    assert(b > a);
    assert(b >= a);
    assert(a != b);
    assert(a == 3);
    assert(top > max - top);
    assert(max > top);
    assert(!(max < top));
    let sa: i34 = a as i34;
    let neg: i34 = 0 - sa;
    let smin: i34 = -8589934592;
    let smax: i34 = 8589934591;
    assert(neg == -3);
    assert(neg + sa == 0);
    assert(neg * neg == 9);
    assert(neg * sa == -9);
    assert(smin + 1 == -8589934591);
    assert(smax + smin == -1);
    assert(smax - sa == 8589934588);
    assert(smin / sa == -2863311530);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 33 == -1);
    assert(neg < sa);
    assert(smin < neg);
    assert(smax > sa);
    assert(smin < smax);
    assert(neg as u34 == 17179869181);
    assert(smin as u34 == top);
    assert(smax as u34 == top - 1);
    assert(neg as i64 == -3);
    assert(neg as u8 == 253);
    assert(neg as i8 == -3);
    assert((neg as i8) as i34 == neg);
    assert(smin as i64 == -8589934592);
    assert(u34::max_value() == max);
    assert(u34::min_value() == 0);
    assert(u34::bits() == 34);
    assert(i34::max_value() == smax);
    assert(i34::min_value() == smin);
    assert(i34::bits() == 34);
    let zero_u: u34 = std::default::Default::default();
    let zero_i: i34 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 17179869176);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 33)
}

fn main(a: u8, b: u8) -> pub u8 {
    check(a as u34, b as u34) as u8
}
