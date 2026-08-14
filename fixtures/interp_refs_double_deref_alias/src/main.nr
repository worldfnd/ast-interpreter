struct Nested {
    y: &mut &mut Field,
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
}
