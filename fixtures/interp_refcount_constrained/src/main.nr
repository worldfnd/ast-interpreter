// Arrays and vectors have a reference count only in unconstrained code; everywhere else the
// builtins return zero, however the value is held.
use std::mem::{array_refcount, vector_refcount};

fn borrowed(array: &mut [Field; 3]) -> u32 {
    array[0] = 9;
    array_refcount(*array)
}

fn main() -> pub u32 {
    let mut array = [1, 2, 3];
    let vector = array.as_vector();
    assert(array_refcount(array) == 0);
    assert(vector_refcount(vector) == 0);
    let copy = array;
    assert(array_refcount(copy) == 0);
    assert(borrowed(&mut array) == 0);
    assert(array[0] == 9);
    array_refcount(array) + vector_refcount(vector)
}
