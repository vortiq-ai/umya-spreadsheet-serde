use std::str::FromStr;

use super::super::super::EnumTrait;
#[derive(Clone, Debug, Default)]
pub enum GroupingValues {
    #[default]
    PercentStacked,
    Stacked,
    Standard,
    Clustered,
}

#[cfg(feature = "serde")]
mod serde_impl {
    use serde::{
        Deserialize,
        Deserializer,
        Serialize,
        Serializer,
    };

    use super::GroupingValues;

    #[derive(Serialize, Deserialize)]
    #[serde(rename = "GroupingValues")]
    enum Representation {
        PercentStacked,
        Stacked,
        Standard,
        Clustered,
    }

    impl Serialize for GroupingValues {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let value = match self {
                Self::PercentStacked => Representation::PercentStacked,
                Self::Stacked => Representation::Stacked,
                Self::Standard => Representation::Standard,
                // Old browser codecs only know the original three variants;
                // clustered bars use the existing unstacked representation.
                Self::Clustered if !serializer.is_human_readable() => Representation::Standard,
                Self::Clustered => Representation::Clustered,
            };
            value.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for GroupingValues {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            Ok(match Representation::deserialize(deserializer)? {
                Representation::PercentStacked => Self::PercentStacked,
                Representation::Stacked => Self::Stacked,
                Representation::Standard => Self::Standard,
                Representation::Clustered => Self::Clustered,
            })
        }
    }
}
impl EnumTrait for GroupingValues {
    fn value_string(&self) -> &str {
        match &self {
            Self::PercentStacked => "percentStacked",
            Self::Stacked => "stacked",
            Self::Standard => "standard",
            Self::Clustered => "clustered",
        }
    }
}
impl FromStr for GroupingValues {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "percentStacked" => Ok(Self::PercentStacked),
            "stacked" => Ok(Self::Stacked),
            "standard" => Ok(Self::Standard),
            "clustered" => Ok(Self::Clustered),
            _ => Err(()),
        }
    }
}
