//! Field-property predicates that explain expected cross-field coverage gaps.

use acvm::FieldConfig;

/// A field property a program depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capability {
    /// Every value of the unsigned `bits`-bit type maps to a distinct field element.
    UnsignedFits(u32),
    /// The two's-complement encoding of the signed `bits`-bit type is injective.
    SignedFits(u32),
    /// The modulus has at least `bits` bits.
    FieldBitsAtLeast(u32),
}

impl Capability {
    /// Whether `field` has this property.
    ///
    /// The width predicates are the compiler's own (`FieldConfig::fits_unsigned`), so a gap
    /// predicted here is one the compiler's own rule produces rather than a restatement of it.
    pub(crate) fn holds(&self, field: FieldConfig) -> bool {
        match self {
            Capability::UnsignedFits(bits) | Capability::SignedFits(bits) => {
                field.fits_unsigned(*bits)
            }
            Capability::FieldBitsAtLeast(bits) => field.num_bits() >= *bits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acvm::FieldId;

    #[test]
    fn tags_follow_the_field() {
        let bn254 = FieldConfig::new(FieldId::Bn254);
        let goldilocks = FieldConfig::new(FieldId::Goldilocks);
        for tag in [
            Capability::UnsignedFits(64),
            Capability::UnsignedFits(128),
            Capability::SignedFits(64),
            Capability::FieldBitsAtLeast(254),
        ] {
            assert!(tag.holds(bn254), "{tag:?}");
            assert!(!tag.holds(goldilocks), "{tag:?}");
        }
        assert!(Capability::UnsignedFits(32).holds(goldilocks));
        assert!(Capability::SignedFits(32).holds(goldilocks));
        assert!(Capability::FieldBitsAtLeast(64).holds(goldilocks));
    }
}
