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

fn main(a: u36, b: u36) -> pub u36 {
    let top: u36 = 1 << 35;
    let max: u36 = 68719476735;
    assert(top == 34359738368);
    assert(a + b == 8);
    assert(b - a == 2);
    assert(a * b == 15);
    assert(max / b == 13743895347);
    assert(max % b == 0);
    assert(max - top == 34359738367);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!max == 0);
    assert(!a == 68719476732);
    assert((top | a) == 34359738371);
    assert((max & top) == top);
    assert((max ^ top) == 34359738367);
    assert(a << 33 == 25769803776);
    assert(a << 35 == 34359738368);
    assert(top >> 35 == 1);
    assert(max >> 1 == 34359738367);
    assert(max >> 35 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 68719476735);
    assert(max as i36 == -1);
    assert(a as i36 == 3);
    assert((a as u8) as u36 == a);
    assert((12345 as Field) as u36 == 12345);
    assert((a as Field) == 3);
    assert((top as Field) == 34359738368);
    assert((max as Field) == 68719476735);
    assert((max as u64 + 1) as u36 == 0);
    assert(a < b);
    assert(a <= b);
    assert(b > a);
    assert(b >= a);
    assert(a != b);
    assert(a == 3);
    assert(top > max - top);
    assert(max > top);
    assert(!(max < top));
    let sa: i36 = a as i36;
    let neg: i36 = 0 - sa;
    let smin: i36 = -34359738368;
    let smax: i36 = 34359738367;
    assert(neg == -3);
    assert(neg + sa == 0);
    assert(neg * neg == 9);
    assert(neg * sa == -9);
    assert(smin + 1 == -34359738367);
    assert(smax + smin == -1);
    assert(smax - sa == 34359738364);
    assert(smin / sa == -11453246122);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 35 == -1);
    assert(neg < sa);
    assert(smin < neg);
    assert(smax > sa);
    assert(smin < smax);
    assert(neg as u36 == 68719476733);
    assert(smin as u36 == top);
    assert(smax as u36 == top - 1);
    assert(neg as i64 == -3);
    assert(neg as u8 == 253);
    assert(neg as i8 == -3);
    assert((neg as i8) as i36 == neg);
    assert(smin as i64 == -34359738368);
    assert(u36::max_value() == max);
    assert(u36::min_value() == 0);
    assert(u36::bits() == 36);
    assert(i36::max_value() == smax);
    assert(i36::min_value() == smin);
    assert(i36::bits() == 36);
    let zero_u: u36 = std::default::Default::default();
    let zero_i: i36 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 68719476728);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 35)
}
