fn main(x: i32) -> pub i32 {
    match x {
        -1 => {
            assert(true);
            x * 10
        },
        -2 => 100,
        0 => 200,
        2 => 300,
        _ => x * 10,
    }
}
