// Multiple reference levels to one cell. `q1` and `q2` are copies of the *same* inner `&mut`, so a
// write through one is observed through the other. And a `&mut &mut` stored in a struct field must
// deref twice to reach the underlying cell. Patterns lifted from the corpus `references` program.
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
