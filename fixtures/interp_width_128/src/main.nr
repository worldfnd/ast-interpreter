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

fn main(a: u128, b: u128) -> pub u128 {
    let top: u128 = 1 << 127;
    let max: u128 = 340282366920938463463374607431768211455;
    assert(top == 170141183460469231731687303715884105728);
    assert(a + b == 8);
    assert(b - a == 2);
    assert(a * b == 15);
    assert(max / b == 68056473384187692692674921486353642291);
    assert(max % b == 0);
    assert(max - top == 170141183460469231731687303715884105727);
    assert(top - 1 + top == max);
    assert((top - 1) * 2 + 1 == max);
    assert((a & b) == 1);
    assert((a | b) == 7);
    assert((a ^ b) == 6);
    assert(!max == 0);
    assert(!a == 340282366920938463463374607431768211452);
    assert((top | a) == 170141183460469231731687303715884105731);
    assert((max & top) == top);
    assert((max ^ top) == 170141183460469231731687303715884105727);
    assert(a << 125 == 127605887595351923798765477786913079296);
    assert(a << 127 == 170141183460469231731687303715884105728);
    assert(top >> 127 == 1);
    assert(max >> 1 == 170141183460469231731687303715884105727);
    assert(max >> 127 == 1);
    assert((b << 3) >> 3 == b);
    assert(max as u8 == 255);
    assert(max as u32 == 4294967295);
    assert(max as u64 == 18446744073709551615);
    assert(max as i128 == -1);
    assert(a as i128 == 3);
    assert((a as u8) as u128 == a);
    assert((12345 as Field) as u128 == 12345);
    assert(a < b);
    assert(a <= b);
    assert(b > a);
    assert(b >= a);
    assert(a != b);
    assert(a == 3);
    assert(top > max - top);
    assert(max > top);
    assert(!(max < top));
    let sa: i128 = a as i128;
    let neg: i128 = 0 - sa;
    let smin: i128 = -170141183460469231731687303715884105728;
    let smax: i128 = 170141183460469231731687303715884105727;
    assert(neg == -3);
    assert(neg + sa == 0);
    assert(neg * neg == 9);
    assert(neg * sa == -9);
    assert(smin + 1 == -170141183460469231731687303715884105727);
    assert(smax + smin == -1);
    assert(smax - sa == 170141183460469231731687303715884105724);
    assert(smin / sa == -56713727820156410577229101238628035242);
    assert(neg % sa == 0);
    assert(neg >> 1 == -2);
    assert(neg << 1 == -6);
    assert(smin >> 127 == -1);
    assert(neg < sa);
    assert(smin < neg);
    assert(smax > sa);
    assert(smin < smax);
    assert(neg as u128 == 340282366920938463463374607431768211453);
    assert(smin as u128 == top);
    assert(smax as u128 == top - 1);
    assert(neg as i64 == -3);
    assert(neg as u8 == 253);
    assert(neg as i8 == -3);
    assert((neg as i8) as i128 == neg);
    assert(smin as i64 == 0);
    assert(u128::max_value() == max);
    assert(u128::min_value() == 0);
    assert(u128::bits() == 128);
    assert(i128::max_value() == smax);
    assert(i128::min_value() == smin);
    assert(i128::bits() == 128);
    let zero_u: u128 = std::default::Default::default();
    let zero_i: i128 = std::default::Default::default();
    assert(zero_u == 0);
    assert(zero_i == 0);
    assert(arith(a, b) == 3);
    assert(arith(sa, 0 - sa - sa) == 3);
    assert(bits(a, b) == 340282366920938463463374607431768211448);
    assert(shifts(b, 3) == b);
    assert(order(a, b));
    assert(!order(b, a));
    assert(order(neg, sa));
    assert(negate(sa) == neg);
    a * b + (top >> 127)
}
