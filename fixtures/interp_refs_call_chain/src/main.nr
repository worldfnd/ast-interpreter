// A `&mut` reference threaded through a call chain: `main` -> `twice` -> `bump` all mutate the same
// caller-owned cell.
fn bump(r: &mut u64, by: u64) {
    *r = *r + by;
}

fn twice(r: &mut u64) {
    bump(r, 5);
    bump(r, 5);
}

fn main(start: u64) -> pub u64 {
    let mut acc = start;
    twice(&mut acc);
    acc
}
