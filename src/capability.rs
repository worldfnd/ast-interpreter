//! Capability tags: the field properties a program needs, written as predicates the cross-field
//! comparator evaluates against each field. A one-sided coverage gap is expected only where a tag
//! predicts it, so the tag names the reason ("a u64 fits in one field element") instead of a
//! program name in an allowlist. Tags never name a field.

use acvm::{AcirField, FieldElement};
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

/// What the comparator knows about one field. Built from a dump's provenance for each side of a
/// cross-field comparison, or from the compiled-in field for the running build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldDescriptor {
    pub name: String,
    pub modulus: BigUint,
    pub embedded_curve: bool,
}

impl FieldDescriptor {
    /// The embedded-curve property is keyed on the field name until the compiler exports a
    /// run-time field configuration: bn254 carries Grumpkin, no other supported field does.
    pub(crate) fn new(name: &str, modulus: BigUint) -> Self {
        FieldDescriptor {
            name: name.to_string(),
            modulus,
            embedded_curve: name == "bn254",
        }
    }

    /// The field this build compiled in.
    pub(crate) fn current() -> Self {
        Self::new(super::corpus::field_tag(), FieldElement::modulus())
    }

    pub(crate) fn bits(&self) -> u64 {
        self.modulus.bits()
    }
}

impl Capability {
    pub(crate) fn holds(&self, field: &FieldDescriptor) -> bool {
        match self {
            Capability::UnsignedFits(bits) | Capability::SignedFits(bits) => {
                (BigUint::one() << *bits as usize) <= field.modulus
            }
            Capability::FieldBitsAtLeast(bits) => field.bits() >= u64::from(*bits),
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
    fn bn254_holds_every_tag() {
        for tag in [
            Capability::UnsignedFits(64),
            Capability::UnsignedFits(128),
            Capability::SignedFits(64),
            Capability::FieldBitsAtLeast(254),
            Capability::EmbeddedCurve,
        ] {
            assert!(tag.holds(&bn254()), "{tag:?}");
        }
    }

    #[test]
    fn goldilocks_holds_only_what_fits_in_64_bits() {
        let gl = goldilocks();
        assert!(Capability::UnsignedFits(32).holds(&gl));
        assert!(Capability::SignedFits(32).holds(&gl));
        assert!(Capability::FieldBitsAtLeast(64).holds(&gl));
        // 2^64 > p = 2^64 - 2^32 + 1: a u64 does not fit and an i64 encoding collides.
        assert!(!Capability::UnsignedFits(64).holds(&gl));
        assert!(!Capability::SignedFits(64).holds(&gl));
        assert!(!Capability::FieldBitsAtLeast(65).holds(&gl));
        assert!(!Capability::EmbeddedCurve.holds(&gl));
    }

    #[test]
    fn the_compiled_in_field_is_described_by_its_tag_and_modulus() {
        let current = FieldDescriptor::current();
        if cfg!(feature = "goldilocks") {
            assert_eq!(current, goldilocks());
        } else {
            assert_eq!(current, bn254());
        }
    }

    #[test]
    fn descriptions_name_the_property_never_the_field() {
        for tag in [
            Capability::UnsignedFits(64),
            Capability::SignedFits(64),
            Capability::FieldBitsAtLeast(65),
            Capability::EmbeddedCurve,
        ] {
            let text = tag.describe();
            assert!(
                !text.contains("bn254") && !text.contains("goldilocks"),
                "{text}"
            );
        }
    }
}
