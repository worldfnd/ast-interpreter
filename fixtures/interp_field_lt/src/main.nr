unconstrained fn lt_unconstrained(x: Field, y: Field) -> bool {
    x.lt(y)
}

fn check_lt(x: Field, y: Field, expected: bool) {
    assert(x.lt(y) == expected);
    let hint = unsafe { lt_unconstrained(x, y) };
    assert(hint == expected);
}

fn main(x: Field) -> pub bool {
    check_lt(x, x, false);
    check_lt(x, x + 1, true);
    check_lt(x + 1, x, false);
    check_lt(1, 0x100, true);
    check_lt(0x100, 1, false);
    check_lt(0, -1, true);
    check_lt(x, -1, true);
    check_lt(-1, 0, false);
    check_lt(-2, -1, true);
    check_lt(-1, -2, false);
    x.lt(42)
}
