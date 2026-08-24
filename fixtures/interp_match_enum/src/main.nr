enum Shape {
    Circle(u32),
    Rect(u32, u32),
}

fn main(x: u32) -> pub u32 {
    let s = if x == 0 { Shape::Circle(7) } else { Shape::Rect(x, x + 1) };
    match s {
        Shape::Circle(r) => r * 2,
        Shape::Rect(w, h) => w * h,
    }
}
