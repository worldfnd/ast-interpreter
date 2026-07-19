// The assertion fails (3 != 4); its message interpolates a `Field`. The message path renders it
// (triage text, never compared), even though the value path would decline a field interpolation.
fn main(x: Field) {
    assert(x == 4, f"x was {x}");
}
