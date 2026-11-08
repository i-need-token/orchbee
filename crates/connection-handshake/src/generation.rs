//! Protocol generations and inclusive generation ranges, `1 ≤ lowest ≤ highest ≤ 65,535`, and
//! capability sets of at most 256 distinct names in bytewise order.

use std::{collections::BTreeSet, num::NonZeroU16};

use serde::{Deserialize, Serialize};

use crate::{CapabilityName, InvalidValue};

/// The most capability names one list carries.
pub const MAXIMUM_CAPABILITIES: usize = 256;

/// One protocol generation, 1 through 65,535.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct Generation(NonZeroU16);

impl Generation {
    /// The generation `value`, or `None` for 0.
    #[must_use]
    pub const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// The generation number.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

impl TryFrom<u16> for Generation {
    type Error = InvalidValue;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(InvalidValue)
    }
}

impl From<Generation> for u16 {
    fn from(generation: Generation) -> Self {
        generation.get()
    }
}

/// An inclusive, nonempty range of generations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "WireRange", into = "WireRange")]
pub struct GenerationRange {
    lowest: Generation,
    highest: Generation,
}

impl GenerationRange {
    /// The range from `lowest` to `highest`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] when `lowest` is above `highest`.
    pub fn new(lowest: Generation, highest: Generation) -> Result<Self, InvalidValue> {
        if lowest <= highest {
            Ok(Self { lowest, highest })
        } else {
            Err(InvalidValue)
        }
    }

    /// A range the registry declares; evaluated in a constant, an invalid range fails the build.
    pub(crate) const fn declared(lowest: u16, highest: u16) -> Self {
        match (Generation::new(lowest), Generation::new(highest)) {
            (Some(lowest), Some(highest)) if lowest.get() <= highest.get() => {
                Self { lowest, highest }
            }
            _ => panic!("a declared generation range needs 1 <= lowest <= highest"),
        }
    }

    /// The lowest generation in the range.
    #[must_use]
    pub const fn lowest(self) -> Generation {
        self.lowest
    }

    /// The highest generation in the range.
    #[must_use]
    pub const fn highest(self) -> Generation {
        self.highest
    }

    /// Whether `generation` lies in the range.
    #[must_use]
    pub fn contains(self, generation: Generation) -> bool {
        self.lowest <= generation && generation <= self.highest
    }

    /// The highest generation both ranges contain, or `None` when they are disjoint.
    #[must_use]
    pub fn highest_common(self, other: Self) -> Option<Generation> {
        let highest = self.highest.min(other.highest);
        (highest >= self.lowest.max(other.lowest)).then_some(highest)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireRange {
    lowest: u16,
    highest: u16,
}

impl TryFrom<WireRange> for GenerationRange {
    type Error = InvalidValue;

    fn try_from(range: WireRange) -> Result<Self, Self::Error> {
        Self::new(
            Generation::try_from(range.lowest)?,
            Generation::try_from(range.highest)?,
        )
    }
}

impl From<GenerationRange> for WireRange {
    fn from(range: GenerationRange) -> Self {
        Self {
            lowest: range.lowest.get(),
            highest: range.highest.get(),
        }
    }
}

/// At most [`MAXIMUM_CAPABILITIES`] distinct capability names. The wire lists them in strictly
/// ascending bytewise order; a decoder rejects any other order instead of sorting.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "Vec<CapabilityName>")]
pub struct CapabilitySet(BTreeSet<CapabilityName>);

impl CapabilitySet {
    /// The set of `names`, in any order; repeated names collapse.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] for more than [`MAXIMUM_CAPABILITIES`] distinct names.
    pub fn new(names: impl IntoIterator<Item = CapabilityName>) -> Result<Self, InvalidValue> {
        let names = names.into_iter().collect::<BTreeSet<_>>();
        if names.len() <= MAXIMUM_CAPABILITIES {
            Ok(Self(names))
        } else {
            Err(InvalidValue)
        }
    }

    /// Whether the set holds `name`.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|candidate| candidate.as_str() == name)
    }

    /// The names in bytewise order.
    pub fn iter(&self) -> impl Iterator<Item = &CapabilityName> {
        self.0.iter()
    }

    /// The number of names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The names both sets hold.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        Self(self.0.intersection(&other.0).cloned().collect())
    }

    /// Whether every name in this set is in `other`.
    #[must_use]
    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }
}

impl TryFrom<Vec<CapabilityName>> for CapabilitySet {
    type Error = InvalidValue;

    fn try_from(names: Vec<CapabilityName>) -> Result<Self, Self::Error> {
        let ascending = names.windows(2).all(|pair| match pair {
            [earlier, later] => earlier < later,
            _ => false,
        });
        if ascending && names.len() <= MAXIMUM_CAPABILITIES {
            Ok(Self(names.into_iter().collect()))
        } else {
            Err(InvalidValue)
        }
    }
}
