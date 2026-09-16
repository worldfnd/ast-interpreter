// The recorder preserves limb order so values a Goldilocks modulus apart remain distinct.
use std::hash::{Hash, Hasher};

struct Recorder {
    state: Field,
}

impl Hasher for Recorder {
    fn finish(self) -> Field {
        self.state
    }

    fn write(&mut self, input: Field) {
        self.state = self.state * 0x100000001b3 + input;
    }
}

fn hash_of<T>(value: T) -> Field
where
    T: Hash,
{
    let mut hasher = Recorder { state: 0 };
    value.hash(&mut hasher);
    hasher.finish()
}

fn main(x: u64) -> pub bool {
    // The Goldilocks modulus, a legal u64.
    let p: u64 = 18446744069414584321;
    let y: u64 = x + p;
    assert(hash_of(x) != hash_of(y));
    let xw: u128 = x as u128;
    assert(hash_of(xw) != hash_of(xw + (p as u128)));
    assert(hash_of(xw) != hash_of(xw + 18446744073709551616));
    assert(hash_of(xw + 18446744073709551616) != hash_of(xw + (p as u128)));
    let xi: i64 = x as i64;
    let yi: i64 = y as i64;
    assert(yi == -4294967290);
    assert(hash_of(xi) != hash_of(yi));
    hash_of(x) == hash_of(y)
}
