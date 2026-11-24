//! A saved machine's color: its palette, its record and command spellings, and the default a new
//! machine takes. The catalog stores it; the Clients write the machine's name in it.
// module-layer: values

/// The color of a saved machine's name wherever it is written: the Tab Group colors that no Agent
/// state, warning or accent takes, in the order a default is chosen from. This computer has none.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MachineColor {
    Cyan,
    Purple,
    Pink,
    Grey,
}
impl MachineColor {
    pub const PALETTE: [Self; 4] = [Self::Cyan, Self::Purple, Self::Pink, Self::Grey];
    /// The record's and every command's spellings, the same in every locale, in palette order.
    pub const NAMES: [&'static str; 4] = ["cyan", "purple", "pink", "grey"];
    pub const fn as_str(self) -> &'static str {
        Self::NAMES[self as usize]
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::PALETTE
            .into_iter()
            .find(|color| color.as_str() == value)
    }
    /// The color the fewest of `used` take; among equals, the earliest in the palette.
    pub fn least_used(used: impl IntoIterator<Item = Self>) -> Self {
        let mut counts = [0_usize; 4];
        for color in used {
            counts[color as usize] += 1;
        }
        Self::PALETTE
            .into_iter()
            .min_by_key(|color| counts[*color as usize])
            .unwrap_or(Self::Cyan)
    }
}
