// Self-checking interpreter fixture: every `assert` encodes a known-correct result, so a clean run
// means the AST computes what the source claims. Covers native integer arithmetic (including a
// constant at the Goldilocks modulus), calls, control flow, arrays, tuples, and a cast.

fn double(x: u64) -> u64 {
    x * 2
}

fn main() {
    // u64 arithmetic reaching the Goldilocks modulus p = 2^64 - 2^32 + 1 (a legal u64, since
    // p < 2^64); as a field element p would wrap to 0, so this checks native integer semantics.
    let p_minus_one: u64 = 18446744069414584320; // p - 1
    assert(p_minus_one + 1 == 18446744069414584321);

    assert(double(21) == 42);

    // 0 + 1 + ... + 9 == 45
    let mut sum: u32 = 0;
    for i in 0..10 {
        sum = sum + i;
    }
    assert(sum == 45);

    let mut arr: [u32; 3] = [10, 20, 30];
    arr[1] = 99;
    assert(arr[1] == 99);
    assert(arr[2] == 30);

    let t: (u32, u64) = (7, 100);
    assert(t.0 == 7);
    assert(t.1 == 100);

    // u64 -> Field cast: a field-dependent value, but the assert holds under any field.
    let f: Field = p_minus_one as Field;
    assert(f == 18446744069414584320);

    let c: u32 = if sum > 40 { 1 } else { 0 };
    assert(c == 1);
}
