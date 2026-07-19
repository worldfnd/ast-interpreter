// A `&mut` parameter mutates a caller local, and a local `&mut` writes through a dereference.
fn add_one(r: &mut u32) {
    *r = *r + 1;
}

fn main() {
    let mut x: u32 = 41;
    let p = &mut x;
    *p = *p + 1;
    assert(x == 42);
    add_one(&mut x);
    assert(x == 43);
}
