// A field of a reference used as a `&mut self` receiver or behind `&mut`, one and two reference
// levels deep: the callee must see the caller's cell, not a copy of the field.
struct BV {
    storage: [Field; 2],
    len: u32,
}

impl BV {
    fn push(&mut self, e: Field) {
        self.storage[self.len] = e;
        self.len += 1;
    }
}

struct FB {
    pcr: BV,
    counter: Field,
}

impl FB {
    fn next(&mut self) -> Field {
        let c = self.counter;
        self.counter += 1;
        c
    }

    fn inner(&mut self) {
        self.pcr.push(self.next());
    }

    fn twice(&mut self) {
        for _ in 0..2 {
            self.inner();
        }
    }
}

struct Bar {
    x: Field,
}

impl Bar {
    fn bump(&mut self) {
        self.x += 1;
    }
}

struct Foo {
    bar: Bar,
    tag: u32,
}

struct Holder {
    inner: Foo,
}

fn through_two_levels(h: &mut &mut Holder) {
    h.inner.bar.bump();
    h.inner.tag += 10;
    let field = &mut h.inner;
    let foo: &mut Foo = *field;
    foo.bar.x += 100;
}

fn main() {
    let mut fb = FB { pcr: BV { storage: [0; 2], len: 0 }, counter: 7 };
    fb.inner();
    assert(fb.pcr.storage[0] == 7);
    assert(fb.pcr.len == 1);
    assert(fb.counter == 8);

    let mut again = FB { pcr: BV { storage: [0; 2], len: 0 }, counter: 3 };
    again.twice();
    assert(again.pcr.storage == [3, 4]);
    assert(again.pcr.len == 2);
    assert(again.counter == 5);

    let foo = &mut Foo { bar: Bar { x: 0 }, tag: 0 };
    foo.bar.bump();
    assert(foo.bar.x == 1);

    let mut fresh = Holder { inner: Foo { bar: Bar { x: 2 }, tag: 0 } };
    let mut r = &mut fresh;
    through_two_levels(&mut r);
    assert(fresh.inner.bar.x == 103);
    assert(fresh.inner.tag == 10);
    assert(r.inner.tag == 10);
}
