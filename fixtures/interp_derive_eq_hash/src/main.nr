use std::hash::{Hash, Hasher};

#[derive(Eq, Hash)]
struct Point {
    x: u32,
    y: u64,
}

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

fn main(x: u32) -> pub bool {
    let a = Point { x, y: 7 };
    let b = Point { x: 3, y: 7 };
    let c = Point { x, y: 8 };
    assert(a == b);
    assert(a != c);
    assert(hash_of(a) == hash_of(b));
    assert(hash_of(a) != hash_of(c));
    a == c
}
