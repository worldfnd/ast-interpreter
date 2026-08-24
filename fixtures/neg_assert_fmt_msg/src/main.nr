struct Point {
    x: u32,
    y: bool,
}

enum Choice {
    None,
    Some(u32),
}

fn main() {
    let mut sum: u32 = 0;
    for i in 0..10 {
        sum += i;
    }
    let field: Field = 3;
    let array: [u32; 2] = [1, 2];
    let tuple: (u32,) = (7,);
    let point = Point { x: 4, y: true };
    let choice = Choice::Some(9);
    assert(sum == 46, f"sum={sum} field={field} array={array} tuple={tuple} point={point} choice={choice}");
}
