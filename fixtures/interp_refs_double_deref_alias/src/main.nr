struct Nested {
    y: &mut &mut Field,
}

struct Pair {
    a: Field,
    b: Field,
}

fn add1(x: &mut Field) {
    *x += 1;
}

fn main() {
    let q = &mut &mut 0;
    let q1 = *q;
    let q2 = *q;
    *q1 = 10;
    *q2 = 2;
    assert(*q1 == 2);
    assert(*q2 == 2);

    let nested = Nested { y: &mut &mut 0 };
    add1(*nested.y);
    assert(**nested.y == 1);

    let mut pair = Pair { a: 1, b: 2 };
    let pair_ref = &mut pair;
    let field_ref = &mut (*pair_ref).b;
    *field_ref = 9;
    assert(pair.b == 9);
}
