#[derive(Eq)]
struct Point {
    x: u32,
    y: u32,
}

fn main() {
    let a = [1, 2, 3];
    let b = [1, 2, 3];
    let c = [1, 2, 4];
    assert(a == b);
    assert(a != c);

    let t1 = (1, true, 5);
    let t2 = (1, true, 5);
    assert(t1 == t2);

    let p1 = Point { x: 3, y: 4 };
    let p2 = Point { x: 3, y: 4 };
    let p3 = Point { x: 3, y: 5 };
    assert(p1 == p2);
    assert(p1 != p3);
}
