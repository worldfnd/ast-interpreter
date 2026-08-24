struct S {
    zeta: u32,
    alpha: u32,
}

global DEFAULT: S = S { zeta: 10, alpha: 20 };

fn main() {
    let mut s = S { zeta: 10, alpha: 20 };
    let r = &mut s.zeta;
    *r = 99;
    assert(s.zeta == 99);
    assert(s.alpha == 20);

    let b = &mut s.alpha;
    s = S { zeta: 1, alpha: 2 };
    assert(*b == 2);

    let mut repeated = [S { zeta: 3, alpha: 4 }; 2];
    repeated[0].zeta = 5;
    assert(repeated[1].zeta == 3);

    let mut global_copy = DEFAULT;
    global_copy.zeta = 30;
    assert(DEFAULT.zeta == 10);
}
