// Negative fixture for the validation frontend's error boundary: `main`'s body has a type error in
// reachable code (a `u64` bound to a `bool`). Reachable errors must not yield a clean mono-AST, or
// the interpreter would validate a silently-miscompiled program.
fn main(x: u64) -> pub u64 {
    let _b: bool = x; // deliberate type error: u64 is not bool
    x
}
