//! Field-property predicates used to explain expected cross-field coverage gaps.

use num_bigint::BigUint;
use num_traits::One;

/// A field property a program depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capability {
    /// Every value of the unsigned `bits`-bit type is its own field element: `2^bits <= p`.
    UnsignedFits(u8),
    /// The two's-complement encoding of the signed `bits`-bit type is injective: `2^bits <= p`.
    SignedFits(u8),
    /// The modulus has at least `bits` bits.
    FieldBitsAtLeast(u32),
    /// The field has a native embedded curve.
    EmbeddedCurve,
}

/// One side of a cross-field comparison, built from a dump's provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldDescriptor {
    pub name: String,
    pub modulus: BigUint,
    pub embedded_curve: bool,
}

impl FieldDescriptor {
    /// The embedded curve is keyed on the name until the compiler exports a run-time field config.
    pub(crate) fn new(name: &str, modulus: BigUint) -> Self {
        FieldDescriptor {
            name: name.to_string(),
            modulus,
            embedded_curve: name == "bn254",
        }
    }
}

impl Capability {
    pub(crate) fn holds(&self, field: &FieldDescriptor) -> bool {
        match self {
            Capability::UnsignedFits(bits) | Capability::SignedFits(bits) => {
                (BigUint::one() << *bits as usize) <= field.modulus
            }
            Capability::FieldBitsAtLeast(bits) => field.modulus.bits() >= u64::from(*bits),
            Capability::EmbeddedCurve => field.embedded_curve,
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Capability::UnsignedFits(bits) => format!("u{bits} fits in one field element"),
            Capability::SignedFits(bits) => format!("i{bits} encodes injectively in the field"),
            Capability::FieldBitsAtLeast(bits) => format!("field has at least {bits} bits"),
            Capability::EmbeddedCurve => "field has an embedded curve".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bn254() -> FieldDescriptor {
        FieldDescriptor::new(
            "bn254",
            "21888242871839275222246405745257275088548364400416034343698204186575808495617"
                .parse()
                .unwrap(),
        )
    }

    fn goldilocks() -> FieldDescriptor {
        FieldDescriptor::new("goldilocks", "18446744069414584321".parse().unwrap())
    }

    #[test]
    fn tags_follow_the_modulus() {
        let (bn, gl) = (bn254(), goldilocks());
        for tag in [
            Capability::UnsignedFits(64),
            Capability::UnsignedFits(128),
            Capability::SignedFits(64),
            Capability::FieldBitsAtLeast(254),
            Capability::EmbeddedCurve,
        ] {
            assert!(tag.holds(&bn), "{tag:?}");
            assert!(!tag.holds(&gl), "{tag:?}");
        }
        assert!(Capability::UnsignedFits(32).holds(&gl));
        assert!(Capability::SignedFits(32).holds(&gl));
        assert!(Capability::FieldBitsAtLeast(64).holds(&gl));
    }
}
