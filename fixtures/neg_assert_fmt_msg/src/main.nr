// The assertion fails (0+1+...+9 = 45, not 46); its format-string message interpolates a
// field-independent `u32`, so the interpreter renders it.
fn main() {
    let mut sum: u32 = 0;
    for i in 0..10 {
        sum += i;
    }
    assert(sum == 46, f"sum was {sum}");
}
