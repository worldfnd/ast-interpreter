// Negative fixture: the interpreter must detect a false assertion. The loop sum is 45 but the
// assert claims 46; the sum is computed (not const-folded), so it compiles and only fails at
// interpretation - the oracle signal we rely on.
fn main() {
    let mut sum: u32 = 0;
    for i in 0..10 {
        sum = sum + i;
    }
    assert(sum == 46); // wrong on purpose
}
