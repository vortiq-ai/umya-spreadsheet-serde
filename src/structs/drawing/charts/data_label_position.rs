// Values of ST_DLblPos. The optional `val` attribute is represented separately
// by the owning scalar element; this enum does not invent an absent default.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataLabelPosition {
    BestFit,
    Bottom,
    Center,
    InsideBase,
    InsideEnd,
    Left,
    OutsideEnd,
    Right,
    Top,
}

impl DataLabelPosition {
    #[must_use]
    pub fn from_xml(value: &str) -> Option<Self> {
        Some(match value {
            "bestFit" => Self::BestFit,
            "b" => Self::Bottom,
            "ctr" => Self::Center,
            "inBase" => Self::InsideBase,
            "inEnd" => Self::InsideEnd,
            "l" => Self::Left,
            "outEnd" => Self::OutsideEnd,
            "r" => Self::Right,
            "t" => Self::Top,
            _ => return None,
        })
    }

    #[must_use]
    pub fn as_xml(self) -> &'static str {
        match self {
            Self::BestFit => "bestFit",
            Self::Bottom => "b",
            Self::Center => "ctr",
            Self::InsideBase => "inBase",
            Self::InsideEnd => "inEnd",
            Self::Left => "l",
            Self::OutsideEnd => "outEnd",
            Self::Right => "r",
            Self::Top => "t",
        }
    }
}
