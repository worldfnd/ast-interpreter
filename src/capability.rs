//! Field-property predicates that explain expected cross-field coverage gaps.

use num_bigint::BigUint;
use num_traits::One;

/// A field property a program depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capability {
    /// Every value of the unsigned `bits`-bit type maps to a distinct field element: `2^bits <= p`.
    UnsignedFits(u8),
    /// The two's-complement encoding of the signed `bits`-bit type is injective: `2^bits <= p`.
    SignedFits(u8),
    /// The modulus has at least `bits` bits.
    FieldBitsAtLeast(u32),
}

impl Capability {
    pub(crate) fn holds(&self, modulus: &BigUint) -> bool {
        match self {
            Capability::UnsignedFits(bits) | Capability::SignedFits(bits) => {
                (BigUint::one() << usize::from(*bits)) <= *modulus
            }
            Capability::FieldBitsAtLeast(bits) => modulus.bits() >= u64::from(*bits),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_follow_the_modulus() {
        let bn254: BigUint =
            "21888242871839275222246405745257275088548364400416034343698204186575808495617"
                .parse()
                .unwrap();
        let goldilocks: BigUint = "18446744069414584321".parse().unwrap();
        for tag in [
            Capability::UnsignedFits(64),
            Capability::UnsignedFits(128),
            Capability::SignedFits(64),
            Capability::FieldBitsAtLeast(254),
        ] {
            assert!(tag.holds(&bn254), "{tag:?}");
            assert!(!tag.holds(&goldilocks), "{tag:?}");
        }
        assert!(Capability::UnsignedFits(32).holds(&goldilocks));
        assert!(Capability::SignedFits(32).holds(&goldilocks));
        assert!(Capability::FieldBitsAtLeast(64).holds(&goldilocks));
    }
}
