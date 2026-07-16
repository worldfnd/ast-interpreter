// A field-independent computation fed entirely from Prover.toml: an array input, a helper call, a
// loop with indexing, and a signed conditional. Exercises the input bridge and interpreter together
// on more than a single scalar, so the result depends on every input being decoded and bound.
fn weighted_sum(xs: [u32; 4]) -> u32 {
    let mut acc: u32 = 0;
    for i in 0..4 {
        acc = acc + xs[i] * (i + 1);
    }
    acc
}

fn main(xs: [u32; 4], k: i16) -> pub u32 {
    let base = weighted_sum(xs);
    if k < 0 {
        base - 5
    } else {
        base + 5
    }
}
