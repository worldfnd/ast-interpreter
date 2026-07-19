// Closures need no special handling: the monomorphizer lowers a closure call to extracting the
// function from its `(env, fn)` tuple and prepending the captured environment, so the existing
// tuple + call machinery runs them. Covers a lambda through a call, a capturing closure, and a HOF.
fn apply(f: fn(u32) -> u32, x: u32) -> u32 {
    f(x)
}

fn main() {
    assert(apply(|x| x * 2, 5) == 10);

    let base: u32 = 100;
    let add_base = |x: u32| x + base;
    assert(add_base(7) == 107);

    let v: [u32] = [1, 2, 3].as_vector();
    assert(v.fold(0, |a: u32, x: u32| a + x) == 6);
}
