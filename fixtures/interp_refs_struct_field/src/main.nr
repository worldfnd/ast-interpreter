// `&mut s.field` aliases one field cell (fields deliberately out of alphabetical order); a sibling
// field is untouched, and a live field reference survives a whole-struct reassignment.
struct S {
    zeta: u32,
    alpha: u32,
}

fn main() {
    let mut s = S { zeta: 10, alpha: 20 };
    let r = &mut s.zeta;
    *r = 99;
    assert(s.zeta == 99);
    assert(s.alpha == 20);

    let b = &mut s.alpha;
    s = S { zeta: 1, alpha: 2 };
    assert(*b == 2);
}
