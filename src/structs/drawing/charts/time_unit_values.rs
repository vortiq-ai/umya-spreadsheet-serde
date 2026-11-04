use std::str::FromStr;

use super::super::super::EnumTrait;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeUnitValues {
    #[default]
    Days,
    Months,
    Years,
}
impl EnumTrait for TimeUnitValues {
    fn value_string(&self) -> &str {
        match &self {
            Self::Days => "days",
            Self::Months => "months",
            Self::Years => "years",
        }
    }
}
impl FromStr for TimeUnitValues {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "days" => Ok(Self::Days),
            "months" => Ok(Self::Months),
            "years" => Ok(Self::Years),
            _ => Err(()),
        }
    }
}
