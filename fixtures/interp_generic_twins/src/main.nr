// The recorder keeps every `write` distinct, so the hash of a `u64` says which `Hash` impl ran:
// bn254's writes the value in one field element; the field-generic twin writes two 32-bit limbs.
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

fn main(x: u32) -> pub Field {
    let wide: u64 = (x as u64) + 0x100000000;
    let mut hasher = Recorder { state: 0 };
    wide.hash(&mut hasher);
    hasher.finish()
}
