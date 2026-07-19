// A literal-integer switch with a wildcard default. Exercises `Constructor::Int` equality (including
// a negative signed literal) and the `default_case` fallback.
fn main(x: i32) -> pub i32 {
    match x {
        -2 => 100,
        0 => 200,
        2 => 300,
        _ => x * 10,
    }
}
