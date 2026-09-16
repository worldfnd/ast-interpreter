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

fn main(a: u126, b: u126) -> pub u126 {
    let top: u126 = 1 << 125;
    let max: u126 = 85070591730234615865843651857942052863;
    assert(top == 42535295865117307932921825928971026432);
    assert(a + b == 8);
    assert(b - a == 2);
    assert(a * b == 15);
    assert(max / b == 17014118346046923173168730371588410572);
    assert(max % b == 3);
    assert(max - top == 42535295865117307932921825928971026431);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!max == 0);
    assert(!a == 85070591730234615865843651857942052860);
    assert((top | a) == 42535295865117307932921825928971026435);
    assert((max & top) == top);
    assert((max ^ top) == 42535295865117307932921825928971026431);
    assert(a << 123 == 31901471898837980949691369446728269824);
    assert(a << 125 == 42535295865117307932921825928971026432);
    assert(top >> 125 == 1);
    assert(max >> 1 == 42535295865117307932921825928971026431);
    assert(max >> 125 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 18446744073709551615);
    assert(max as i126 == -1);
    assert(a as i126 == 3);
    assert((a as u8) as u126 == a);
    assert((12345 as Field) as u126 == 12345);
    assert((top as u128) == 42535295865117307932921825928971026432);
    assert((a as u128) as u126 == a);
    assert(a < b);
    assert(a <= b);
    assert(b > a);
    assert(b >= a);
    assert(a != b);
    assert(a == 3);
    assert(top > max - top);
    assert(max > top);
    assert(!(max < top));
    let sa: i126 = a as i126;
    let neg: i126 = 0 - sa;
    let smin: i126 = -42535295865117307932921825928971026432;
    let smax: i126 = 42535295865117307932921825928971026431;
    assert(neg == -3);
    assert(neg + sa == 0);
    assert(neg * neg == 9);
    assert(neg * sa == -9);
    assert(smin + 1 == -42535295865117307932921825928971026431);
    assert(smax + smin == -1);
    assert(smax - sa == 42535295865117307932921825928971026428);
    assert(smin / sa == -14178431955039102644307275309657008810);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 125 == -1);
    assert(neg < sa);
    assert(smin < neg);
    assert(smax > sa);
    assert(smin < smax);
    assert(neg as u126 == 85070591730234615865843651857942052861);
    assert(smin as u126 == top);
    assert(smax as u126 == top - 1);
    assert(neg as i64 == -3);
    assert(neg as u8 == 253);
    assert(neg as i8 == -3);
    assert((neg as i8) as i126 == neg);
    assert(smin as i64 == 0);
    assert(u126::max_value() == max);
    assert(u126::min_value() == 0);
    assert(u126::bits() == 126);
    assert(i126::max_value() == smax);
    assert(i126::min_value() == smin);
    assert(i126::bits() == 126);
    let zero_u: u126 = std::default::Default::default();
    let zero_i: i126 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 85070591730234615865843651857942052856);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 125)
}
