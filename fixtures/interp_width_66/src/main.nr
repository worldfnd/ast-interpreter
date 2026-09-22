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

fn main(a: u66, b: u66) -> pub u66 {
    let top: u66 = 1 << 65;
    let max: u66 = 73786976294838206463;
    assert(top == 36893488147419103232);
    assert(a + b == 8);
    assert(b - a == 2);
    assert(a * b == 15);
    assert(max / b == 14757395258967641292);
    assert(max % b == 3);
    assert(max - top == 36893488147419103231);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!max == 0);
    assert(!a == 73786976294838206460);
    assert((top | a) == 36893488147419103235);
    assert((max & top) == top);
    assert((max ^ top) == 36893488147419103231);
    assert(a << 63 == 27670116110564327424);
    assert(a << 65 == 36893488147419103232);
    assert(top >> 65 == 1);
    assert(max >> 1 == 36893488147419103231);
    assert(max >> 65 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 18446744073709551615);
    assert(max as i66 == -1);
    assert(a as i66 == 3);
    assert((a as u8) as u66 == a);
    assert((12345 as Field) as u66 == 12345);
    assert((top as u128) == 36893488147419103232);
    assert((a as u128) as u66 == a);
    assert(a < b);
    assert(a <= b);
    assert(b > a);
    assert(b >= a);
    assert(a != b);
    assert(a == 3);
    assert(top > max - top);
    assert(max > top);
    assert(!(max < top));
    let sa: i66 = a as i66;
    let neg: i66 = 0 - sa;
    let smin: i66 = -36893488147419103232;
    let smax: i66 = 36893488147419103231;
    assert(neg == -3);
    assert(neg + sa == 0);
    assert(neg * neg == 9);
    assert(neg * sa == -9);
    assert(smin + 1 == -36893488147419103231);
    assert(smax + smin == -1);
    assert(smax - sa == 36893488147419103228);
    assert(smin / sa == -12297829382473034410);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 65 == -1);
    assert(neg < sa);
    assert(smin < neg);
    assert(smax > sa);
    assert(smin < smax);
    assert(neg as u66 == 73786976294838206461);
    assert(smin as u66 == top);
    assert(smax as u66 == top - 1);
    assert(neg as i64 == -3);
    assert(neg as u8 == 253);
    assert(neg as i8 == -3);
    assert((neg as i8) as i66 == neg);
    assert(smin as i64 == 0);
    assert(u66::max_value() == max);
    assert(u66::min_value() == 0);
    assert(u66::bits() == 66);
    assert(i66::max_value() == smax);
    assert(i66::min_value() == smin);
    assert(i66::bits() == 66);
    let zero_u: u66 = std::default::Default::default();
    let zero_i: i66 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 73786976294838206456);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 65)
}
