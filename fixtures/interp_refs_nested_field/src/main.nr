// A reference stored in a struct field. Taking `&mut holder.inner_ref.v` must peel through the
// ref-typed field to reach the live cell, so mutating through it aliases the original `inner` and
// leaves the sibling field untouched. This needs eval_place to follow more than one reference layer
// (struct field -> `&mut Inner` -> field), the case the single-level place path used to reject.
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
