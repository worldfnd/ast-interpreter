fn bump(r: &mut u64, by: u64) {
    *r = *r + by;
}

fn twice(r: &mut u64) {
    bump(r, 5);
    bump(r, 5);
}

fn main(start: u32) -> pub u32 {
    let mut acc = start as u64;
    twice(&mut acc);
    acc as u32
}
