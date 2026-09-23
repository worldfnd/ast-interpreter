// Negative fixture for the validation frontend's error boundary: `main`'s body has a type error in
// reachable code (a `u32` bound to a `bool`). Reachable errors must not yield a clean mono-AST, or
// the interpreter would validate a silently-miscompiled program.
fn main(x: u32) -> pub u32 {
    let _b: bool = x; // deliberate type error: u32 is not bool
    x
}
