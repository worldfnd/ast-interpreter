// Field-independent hint builtins that survive to a runtime call: `black_box` is the identity,
// `as_witness` and `assert_constant` are no-ops. (`zeroed` is deliberately absent: the monomorphizer
// const-folds it into a literal before the interpreter ever sees a call.)
fn main() {
    let x: u32 = std::hint::black_box(7);
    assert(x == 7);

    let arr: [u32; 3] = std::hint::black_box([1, 2, 3]);
    assert(arr[0] + arr[1] + arr[2] == 6);

    let f: Field = 9;
    std::as_witness(f);
    assert(f == 9);

    std::assert_constant(3);
    assert(x + 3 == 10);
}
