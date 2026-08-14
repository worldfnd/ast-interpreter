struct Inner {
    v: u32,
    w: u32,
}

struct Holder {
    inner_ref: &mut Inner,
}

fn main() {
    let mut inner = Inner { v: 1, w: 2 };
    let holder = Holder { inner_ref: &mut inner };

    let r = &mut holder.inner_ref.v;
    *r = 42;

    assert(inner.v == 42);
    assert(inner.w == 2);
    assert(holder.inner_ref.v == 42);
    assert(holder.inner_ref.w == 2);
}
