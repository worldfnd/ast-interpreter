// Struct fields are deliberately out of alphabetical order at both nesting levels, so an input
// bridge that mapped the alphabetical ABI map onto the lowered tuple positionally would misplace
// them. Each field is weighted distinctly, so any swap changes the result.
struct Pair {
    zeta: u8,
    alpha: u8,
}

struct Outer {
    tail: [u8; 3],
    head: Pair,
}

fn main(s: Outer) -> pub u32 {
    let sum = s.tail[0] + s.tail[1] + s.tail[2];
    (s.head.zeta as u32) * 1000 + (s.head.alpha as u32) * 100 + sum as u32
}
