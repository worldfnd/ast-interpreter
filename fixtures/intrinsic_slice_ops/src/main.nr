fn main() {
    let v: [u32] = [10, 20, 30].as_vector();
    assert(v.len() == 3);

    let v = v.push_back(40);
    assert(v.len() == 4);
    assert(v[3] == 40);

    let v = v.push_front(5);
    assert(v.len() == 5);
    assert(v[0] == 5);

    let (v, last) = v.pop_back();
    assert(last == 40);
    assert(v.len() == 4);

    let (first, v) = v.pop_front();
    assert(first == 5);
    assert(v.len() == 3);

    let v = v.insert(1, 99);
    assert(v.len() == 4);
    assert(v[1] == 99);

    let (v, removed) = v.remove(1);
    assert(removed == 99);
    assert(v.len() == 3);
    assert(v[0] == 10);
    assert(v[2] == 30);
}
