// Both fields run this, but Goldilocks cannot parse the recorded i64 return of -1.
fn main(x: u8) -> pub i64 {
    -(x as i64)
}
