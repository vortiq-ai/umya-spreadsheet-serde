//! Typed `ChartEx` chart, layout, axis, label and geography content.
use std::io::{
    BufRead,
    Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    data::{
        ChartExAutomaticNumber,
        ChartExAxisStep,
        ChartExBinary,
        ChartExBindingType,
        ChartExColorPositionCount,
        ChartExData,
        ChartExExtensionList,
        ChartExGapWidth,
        ChartExInteger,
        ChartExTextData,
    },
    xml::{
        self,
        AttributeValue,
        Attributes,
        NativeElement,
        Presence,
        WriteContext,
        children,
        native_element,
        start,
    },
};
use crate::{
    drawing::{
        SolidFill,
        charts::{
            Formula,
            NamespaceScope,
            ShapeProperties,
            TextProperties,
            XmlDouble,
        },
    },
    reader::xlsx::SourceCacheSession,
    writer::driver::write_end_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExExtensionDropMode {
    #[default]
    Never,
    OnModelChange,
    OnDataChange,
    Always,
}
impl ChartExExtensionDropMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::OnModelChange => "onModelChange",
            Self::OnDataChange => "onDataChange",
            Self::Always => "always",
        }
    }
}
impl AttributeValue for ChartExExtensionDropMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "never" => Ok(Self::Never),
            "onModelChange" => Ok(Self::OnModelChange),
            "onDataChange" => Ok(Self::OnDataChange),
            "always" => Ok(Self::Always),
            _ => Err(format!("invalid ChartExExtensionDropMode: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExExtensionDropMode {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExStringDimensionType {
    #[default]
    Category,
    ColorStr,
    EntityId,
}
impl ChartExStringDimensionType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Category => "cat",
            Self::ColorStr => "colorStr",
            Self::EntityId => "entityId",
        }
    }
}
impl AttributeValue for ChartExStringDimensionType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cat" => Ok(Self::Category),
            "colorStr" => Ok(Self::ColorStr),
            "entityId" => Ok(Self::EntityId),
            _ => Err(format!("invalid ChartExStringDimensionType: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExStringDimensionType {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExNumericDimensionType {
    #[default]
    Value,
    X,
    Y,
    Size,
    ColorVal,
}
impl ChartExNumericDimensionType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Value => "val",
            Self::X => "x",
            Self::Y => "y",
            Self::Size => "size",
            Self::ColorVal => "colorVal",
        }
    }
}
impl AttributeValue for ChartExNumericDimensionType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "val" => Ok(Self::Value),
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            "size" => Ok(Self::Size),
            "colorVal" => Ok(Self::ColorVal),
            _ => Err(format!("invalid ChartExNumericDimensionType: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExNumericDimensionType {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExSidePos {
    #[default]
    Left,
    Top,
    Right,
    Bottom,
}
impl ChartExSidePos {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "l",
            Self::Top => "t",
            Self::Right => "r",
            Self::Bottom => "b",
        }
    }
}
impl AttributeValue for ChartExSidePos {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "l" => Ok(Self::Left),
            "t" => Ok(Self::Top),
            "r" => Ok(Self::Right),
            "b" => Ok(Self::Bottom),
            _ => Err(format!("invalid ChartExSidePos: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExSidePos {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExPosAlign {
    #[default]
    Minimum,
    Center,
    Maximum,
}
impl ChartExPosAlign {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minimum => "min",
            Self::Center => "ctr",
            Self::Maximum => "max",
        }
    }
}
impl AttributeValue for ChartExPosAlign {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "min" => Ok(Self::Minimum),
            "ctr" => Ok(Self::Center),
            "max" => Ok(Self::Maximum),
            _ => Err(format!("invalid ChartExPosAlign: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExPosAlign {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExAxisUnit {
    #[default]
    Hundreds,
    Thousands,
    TenThousands,
    HundredThousands,
    Millions,
    TenMillions,
    HundredMillions,
    Billions,
    Trillions,
    Percentage,
}
impl ChartExAxisUnit {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hundreds => "hundreds",
            Self::Thousands => "thousands",
            Self::TenThousands => "tenThousands",
            Self::HundredThousands => "hundredThousands",
            Self::Millions => "millions",
            Self::TenMillions => "tenMillions",
            Self::HundredMillions => "hundredMillions",
            Self::Billions => "billions",
            Self::Trillions => "trillions",
            Self::Percentage => "percentage",
        }
    }
}
impl AttributeValue for ChartExAxisUnit {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "hundreds" => Ok(Self::Hundreds),
            "thousands" => Ok(Self::Thousands),
            "tenThousands" => Ok(Self::TenThousands),
            "hundredThousands" => Ok(Self::HundredThousands),
            "millions" => Ok(Self::Millions),
            "tenMillions" => Ok(Self::TenMillions),
            "hundredMillions" => Ok(Self::HundredMillions),
            "billions" => Ok(Self::Billions),
            "trillions" => Ok(Self::Trillions),
            "percentage" => Ok(Self::Percentage),
            _ => Err(format!("invalid ChartExAxisUnit: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExAxisUnit {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExTickMarksType {
    #[default]
    In,
    Out,
    Cross,
    None,
}
impl ChartExTickMarksType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::Cross => "cross",
            Self::None => "none",
        }
    }
}
impl AttributeValue for ChartExTickMarksType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "in" => Ok(Self::In),
            "out" => Ok(Self::Out),
            "cross" => Ok(Self::Cross),
            "none" => Ok(Self::None),
            _ => Err(format!("invalid ChartExTickMarksType: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExTickMarksType {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExSeriesLayout {
    #[default]
    BoxWhisker,
    ClusteredColumn,
    Funnel,
    ParetoLine,
    RegionMap,
    Sunburst,
    Treemap,
    Waterfall,
}
impl ChartExSeriesLayout {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BoxWhisker => "boxWhisker",
            Self::ClusteredColumn => "clusteredColumn",
            Self::Funnel => "funnel",
            Self::ParetoLine => "paretoLine",
            Self::RegionMap => "regionMap",
            Self::Sunburst => "sunburst",
            Self::Treemap => "treemap",
            Self::Waterfall => "waterfall",
        }
    }
}
impl AttributeValue for ChartExSeriesLayout {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "boxWhisker" => Ok(Self::BoxWhisker),
            "clusteredColumn" => Ok(Self::ClusteredColumn),
            "funnel" => Ok(Self::Funnel),
            "paretoLine" => Ok(Self::ParetoLine),
            "regionMap" => Ok(Self::RegionMap),
            "sunburst" => Ok(Self::Sunburst),
            "treemap" => Ok(Self::Treemap),
            "waterfall" => Ok(Self::Waterfall),
            _ => Err(format!("invalid ChartExSeriesLayout: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExSeriesLayout {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExParentLabelLayout {
    #[default]
    None,
    Banner,
    Overlapping,
}
impl ChartExParentLabelLayout {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Banner => "banner",
            Self::Overlapping => "overlapping",
        }
    }
}
impl AttributeValue for ChartExParentLabelLayout {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "banner" => Ok(Self::Banner),
            "overlapping" => Ok(Self::Overlapping),
            _ => Err(format!("invalid ChartExParentLabelLayout: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExParentLabelLayout {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExRegionLabelLayout {
    #[default]
    None,
    BestFitOnly,
    ShowAll,
}
impl ChartExRegionLabelLayout {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BestFitOnly => "bestFitOnly",
            Self::ShowAll => "showAll",
        }
    }
}
impl AttributeValue for ChartExRegionLabelLayout {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "bestFitOnly" => Ok(Self::BestFitOnly),
            "showAll" => Ok(Self::ShowAll),
            _ => Err(format!("invalid ChartExRegionLabelLayout: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExRegionLabelLayout {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExIntervalClosedSide {
    #[default]
    Left,
    Right,
}
impl ChartExIntervalClosedSide {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "l",
            Self::Right => "r",
        }
    }
}
impl AttributeValue for ChartExIntervalClosedSide {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "l" => Ok(Self::Left),
            "r" => Ok(Self::Right),
            _ => Err(format!("invalid ChartExIntervalClosedSide: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExIntervalClosedSide {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExEntityType {
    #[default]
    Address,
    AdminDistrict,
    AdminDistrict2,
    AdminDistrict3,
    Continent,
    CountryRegion,
    Locality,
    Ocean,
    Planet,
    PostalCode,
    Region,
    Unsupported,
}
impl ChartExEntityType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Address => "Address",
            Self::AdminDistrict => "AdminDistrict",
            Self::AdminDistrict2 => "AdminDistrict2",
            Self::AdminDistrict3 => "AdminDistrict3",
            Self::Continent => "Continent",
            Self::CountryRegion => "CountryRegion",
            Self::Locality => "Locality",
            Self::Ocean => "Ocean",
            Self::Planet => "Planet",
            Self::PostalCode => "PostalCode",
            Self::Region => "Region",
            Self::Unsupported => "Unsupported",
        }
    }
}
impl AttributeValue for ChartExEntityType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Address" => Ok(Self::Address),
            "AdminDistrict" => Ok(Self::AdminDistrict),
            "AdminDistrict2" => Ok(Self::AdminDistrict2),
            "AdminDistrict3" => Ok(Self::AdminDistrict3),
            "Continent" => Ok(Self::Continent),
            "CountryRegion" => Ok(Self::CountryRegion),
            "Locality" => Ok(Self::Locality),
            "Ocean" => Ok(Self::Ocean),
            "Planet" => Ok(Self::Planet),
            "PostalCode" => Ok(Self::PostalCode),
            "Region" => Ok(Self::Region),
            "Unsupported" => Ok(Self::Unsupported),
            _ => Err(format!("invalid ChartExEntityType: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExEntityType {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExGeoProjectionType {
    #[default]
    Mercator,
    Miller,
    Robinson,
    Albers,
}
impl ChartExGeoProjectionType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mercator => "mercator",
            Self::Miller => "miller",
            Self::Robinson => "robinson",
            Self::Albers => "albers",
        }
    }
}
impl AttributeValue for ChartExGeoProjectionType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "mercator" => Ok(Self::Mercator),
            "miller" => Ok(Self::Miller),
            "robinson" => Ok(Self::Robinson),
            "albers" => Ok(Self::Albers),
            _ => Err(format!("invalid ChartExGeoProjectionType: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExGeoProjectionType {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExGeoMappingLevel {
    #[default]
    DataOnly,
    PostalCode,
    County,
    State,
    CountryRegion,
    CountryRegionList,
    World,
}
impl ChartExGeoMappingLevel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DataOnly => "dataOnly",
            Self::PostalCode => "postalCode",
            Self::County => "county",
            Self::State => "state",
            Self::CountryRegion => "countryRegion",
            Self::CountryRegionList => "countryRegionList",
            Self::World => "world",
        }
    }
}
impl AttributeValue for ChartExGeoMappingLevel {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "dataOnly" => Ok(Self::DataOnly),
            "postalCode" => Ok(Self::PostalCode),
            "county" => Ok(Self::County),
            "state" => Ok(Self::State),
            "countryRegion" => Ok(Self::CountryRegion),
            "countryRegionList" => Ok(Self::CountryRegionList),
            "world" => Ok(Self::World),
            _ => Err(format!("invalid ChartExGeoMappingLevel: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExGeoMappingLevel {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExQuartileMethod {
    #[default]
    Inclusive,
    Exclusive,
}
impl ChartExQuartileMethod {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inclusive => "inclusive",
            Self::Exclusive => "exclusive",
        }
    }
}
impl AttributeValue for ChartExQuartileMethod {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "inclusive" => Ok(Self::Inclusive),
            "exclusive" => Ok(Self::Exclusive),
            _ => Err(format!("invalid ChartExQuartileMethod: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExQuartileMethod {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExDataLabelPos {
    #[default]
    BestFit,
    Bottom,
    Center,
    InBase,
    InEnd,
    Left,
    OutEnd,
    Right,
    Top,
}
impl ChartExDataLabelPos {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BestFit => "bestFit",
            Self::Bottom => "b",
            Self::Center => "ctr",
            Self::InBase => "inBase",
            Self::InEnd => "inEnd",
            Self::Left => "l",
            Self::OutEnd => "outEnd",
            Self::Right => "r",
            Self::Top => "t",
        }
    }
}
impl AttributeValue for ChartExDataLabelPos {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "bestFit" => Ok(Self::BestFit),
            "b" => Ok(Self::Bottom),
            "ctr" => Ok(Self::Center),
            "inBase" => Ok(Self::InBase),
            "inEnd" => Ok(Self::InEnd),
            "l" => Ok(Self::Left),
            "outEnd" => Ok(Self::OutEnd),
            "r" => Ok(Self::Right),
            "t" => Ok(Self::Top),
            _ => Err(format!("invalid ChartExDataLabelPos: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExDataLabelPos {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExArtifactType {
    #[default]
    Html,
}
impl ChartExArtifactType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
        }
    }
}
impl AttributeValue for ChartExArtifactType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "html" => Ok(Self::Html),
            _ => Err(format!("invalid ChartExArtifactType: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExArtifactType {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ChartExPageOrientation {
    #[default]
    Default,
    Portrait,
    Landscape,
}
impl ChartExPageOrientation {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Portrait => "portrait",
            Self::Landscape => "landscape",
        }
    }
}
impl AttributeValue for ChartExPageOrientation {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "default" => Ok(Self::Default),
            "portrait" => Ok(Self::Portrait),
            "landscape" => Ok(Self::Landscape),
            _ => Err(format!("invalid ChartExPageOrientation: {value}")),
        }
    }

    fn lexical(&self) -> Result<String, String> {
        Ok(self.as_str().to_owned())
    }
}
impl NativeElement for ChartExPageOrientation {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        _session: &SourceCacheSession,
    ) -> Result<Self, String> {
        Attributes::read(element, &parent.child(element)?)?.finish()?;
        Self::parse(&xml::text(reader, element, empty)?)
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        _context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        xml::leaf(writer, name, self.as_str());
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExNumberFormat {
    pub format_code:   String,
    pub source_linked: Option<bool>,
}
native_element!(
    ChartExNumberFormat,
    attrs {
        format_code: String => "formatCode" required,
        source_linked: bool => "sourceLinked" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExRelId {
    pub relationship_id: String,
}
native_element!(
    ChartExRelId,
    attrs {
        relationship_id: String => "r:id" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExValueColors {
    pub min_color: Option<SolidFill>,
    pub mid_color: Option<SolidFill>,
    pub max_color: Option<SolidFill>,
}
native_element!(
    ChartExValueColors,
    attrs {
    },
    children {
        min_color: SolidFill => "minColor" one,
        mid_color: SolidFill => "midColor" one,
        max_color: SolidFill => "maxColor" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExExternalData {
    pub relationship_id: String,
    pub auto_update:     Option<bool>,
}
native_element!(
    ChartExExternalData,
    attrs {
        relationship_id: String => "r:id" required,
        auto_update: bool => "autoUpdate" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataId {
    pub value: u32,
}
native_element!(
    ChartExDataId,
    attrs {
        value: u32 => "val" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataCollection {
    pub external_data: Option<ChartExExternalData>,
    pub data:          Vec<ChartExData>,
    pub extensions:    Option<ChartExExtensionList>,
}
native_element!(
    ChartExDataCollection,
    attrs {
    },
    children {
        external_data: ChartExExternalData => "externalData" one,
        data: ChartExData => "data" many,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [data],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExText {
    pub tx_data: Option<ChartExTextData>,
    pub rich:    Option<TextProperties>,
}
native_element!(
    ChartExText,
    attrs {
    },
    children {
        tx_data: ChartExTextData => "txData" one,
        rich: TextProperties => "rich" one,
    },
    required [],
    choices [required [tx_data, rich]]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExOffset {
    pub top:  XmlDouble,
    pub left: XmlDouble,
}
native_element!(
    ChartExOffset,
    attrs {
        top: XmlDouble => "top" required,
        left: XmlDouble => "left" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExChartTitle {
    pub position:         Option<ChartExSidePos>,
    pub align:            Option<ChartExPosAlign>,
    pub overlay:          Option<bool>,
    pub text:             Option<ChartExText>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub offset:           Option<ChartExOffset>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExChartTitle,
    attrs {
        position: ChartExSidePos => "pos" optional,
        align: ChartExPosAlign => "align" optional,
        overlay: bool => "overlay" optional,
    },
    children {
        text: ChartExText => "tx" one,
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        offset: ChartExOffset => "offset" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExCategoryAxisScaling {
    pub gap_width: Option<ChartExGapWidth>,
}
native_element!(
    ChartExCategoryAxisScaling,
    attrs {
        gap_width: ChartExGapWidth => "gapWidth" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExValueAxisScaling {
    pub max:        Option<ChartExAutomaticNumber>,
    pub min:        Option<ChartExAutomaticNumber>,
    pub major_unit: Option<ChartExAxisStep>,
    pub minor_unit: Option<ChartExAxisStep>,
}
native_element!(
    ChartExValueAxisScaling,
    attrs {
        max: ChartExAutomaticNumber => "max" optional,
        min: ChartExAutomaticNumber => "min" optional,
        major_unit: ChartExAxisStep => "majorUnit" optional,
        minor_unit: ChartExAxisStep => "minorUnit" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAxisTitle {
    pub text:             Option<ChartExText>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub offset:           Option<ChartExOffset>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExAxisTitle,
    attrs {
    },
    children {
        text: ChartExText => "tx" one,
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        offset: ChartExOffset => "offset" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAxisUnitsLabel {
    pub text:             Option<ChartExText>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExAxisUnitsLabel,
    attrs {
    },
    children {
        text: ChartExText => "tx" one,
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAxisUnits {
    pub unit:        Option<ChartExAxisUnit>,
    pub units_label: Option<ChartExAxisUnitsLabel>,
    pub extensions:  Option<ChartExExtensionList>,
}
native_element!(
    ChartExAxisUnits,
    attrs {
        unit: ChartExAxisUnit => "unit" optional,
    },
    children {
        units_label: ChartExAxisUnitsLabel => "unitsLabel" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGridlines {
    pub shape_properties: Option<ShapeProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExGridlines,
    attrs {
    },
    children {
        shape_properties: ShapeProperties => "spPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExTickMarks {
    pub kind:       Option<ChartExTickMarksType>,
    pub extensions: Option<ChartExExtensionList>,
}
native_element!(
    ChartExTickMarks,
    attrs {
        kind: ChartExTickMarksType => "type" optional,
    },
    children {
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExTickLabels {
    pub extensions: Option<ChartExExtensionList>,
}
native_element!(
    ChartExTickLabels,
    attrs {
    },
    children {
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAxis {
    pub id:               u32,
    pub hidden:           Option<bool>,
    pub category_scaling: Option<ChartExCategoryAxisScaling>,
    pub value_scaling:    Option<ChartExValueAxisScaling>,
    pub title:            Option<ChartExAxisTitle>,
    pub units:            Option<ChartExAxisUnits>,
    pub major_gridlines:  Option<ChartExGridlines>,
    pub minor_gridlines:  Option<ChartExGridlines>,
    pub major_tick_marks: Option<ChartExTickMarks>,
    pub minor_tick_marks: Option<ChartExTickMarks>,
    pub tick_labels:      Option<ChartExTickLabels>,
    pub number_format:    Option<ChartExNumberFormat>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExAxis,
    attrs {
        id: u32 => "id" required,
        hidden: bool => "hidden" optional,
    },
    children {
        category_scaling: ChartExCategoryAxisScaling => "catScaling" one,
        value_scaling: ChartExValueAxisScaling => "valScaling" one,
        title: ChartExAxisTitle => "title" one,
        units: ChartExAxisUnits => "units" one,
        major_gridlines: ChartExGridlines => "majorGridlines" one,
        minor_gridlines: ChartExGridlines => "minorGridlines" one,
        major_tick_marks: ChartExTickMarks => "majorTickMarks" one,
        minor_tick_marks: ChartExTickMarks => "minorTickMarks" one,
        tick_labels: ChartExTickLabels => "tickLabels" one,
        number_format: ChartExNumberFormat => "numFmt" one,
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices [required [category_scaling,value_scaling]]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExParentLabelProperties {
    pub value: ChartExParentLabelLayout,
}
native_element!(
    ChartExParentLabelProperties,
    attrs {
        value: ChartExParentLabelLayout => "val" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExRegionLabelProperties {
    pub value: ChartExRegionLabelLayout,
}
native_element!(
    ChartExRegionLabelProperties,
    attrs {
        value: ChartExRegionLabelLayout => "val" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExSeriesElementVisibilities {
    pub connector_lines: Option<bool>,
    pub mean_line:       Option<bool>,
    pub mean_marker:     Option<bool>,
    pub nonoutliers:     Option<bool>,
    pub outliers:        Option<bool>,
}
native_element!(
    ChartExSeriesElementVisibilities,
    attrs {
        connector_lines: bool => "connectorLines" optional,
        mean_line: bool => "meanLine" optional,
        mean_marker: bool => "meanMarker" optional,
        nonoutliers: bool => "nonoutliers" optional,
        outliers: bool => "outliers" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAggregation {}
native_element!(
    ChartExAggregation,
    attrs {
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExBinning {
    pub interval_closed: Option<ChartExIntervalClosedSide>,
    pub underflow:       Option<ChartExAutomaticNumber>,
    pub overflow:        Option<ChartExAutomaticNumber>,
    pub bin_size:        Option<XmlDouble>,
    pub bin_count:       Option<u32>,
}
native_element!(
    ChartExBinning,
    attrs {
        interval_closed: ChartExIntervalClosedSide => "intervalClosed" optional,
        underflow: ChartExAutomaticNumber => "underflow" optional,
        overflow: ChartExAutomaticNumber => "overflow" optional,
    },
    children {
        bin_size: XmlDouble => "binSize" one,
        bin_count: u32 => "binCount" one,
    },
    required [],
    choices [optional [bin_size,bin_count]]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoLocationQuery {
    pub country_region:  Option<String>,
    pub admin_district1: Option<String>,
    pub admin_district2: Option<String>,
    pub postal_code:     Option<String>,
    pub entity_type:     ChartExEntityType,
}
native_element!(
    ChartExGeoLocationQuery,
    attrs {
        country_region: String => "countryRegion" optional,
        admin_district1: String => "adminDistrict1" optional,
        admin_district2: String => "adminDistrict2" optional,
        postal_code: String => "postalCode" optional,
        entity_type: ChartExEntityType => "entityType" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExAddress {
    pub address1:         Option<String>,
    pub country_region:   Option<String>,
    pub admin_district1:  Option<String>,
    pub admin_district2:  Option<String>,
    pub postal_code:      Option<String>,
    pub locality:         Option<String>,
    pub iso_country_code: Option<String>,
}
native_element!(
    ChartExAddress,
    attrs {
        address1: String => "address1" optional,
        country_region: String => "countryRegion" optional,
        admin_district1: String => "adminDistrict1" optional,
        admin_district2: String => "adminDistrict2" optional,
        postal_code: String => "postalCode" optional,
        locality: String => "locality" optional,
        iso_country_code: String => "isoCountryCode" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoLocation {
    pub latitude:    Option<XmlDouble>,
    pub longitude:   Option<XmlDouble>,
    pub entity_name: String,
    pub entity_type: ChartExEntityType,
    pub address:     Option<ChartExAddress>,
}
native_element!(
    ChartExGeoLocation,
    attrs {
        latitude: XmlDouble => "latitude" optional,
        longitude: XmlDouble => "longitude" optional,
        entity_name: String => "entityName" required,
        entity_type: ChartExEntityType => "entityType" required,
    },
    children {
        address: ChartExAddress => "address" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoLocations {
    pub geo_location: Option<ChartExGeoLocation>,
}
native_element!(
    ChartExGeoLocations,
    attrs {
    },
    children {
        geo_location: ChartExGeoLocation => "geoLocation" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoLocationQueryResult {
    pub geo_location_query: Option<ChartExGeoLocationQuery>,
    pub geo_locations:      Option<ChartExGeoLocations>,
}
native_element!(
    ChartExGeoLocationQueryResult,
    attrs {
    },
    children {
        geo_location_query: ChartExGeoLocationQuery => "geoLocationQuery" one,
        geo_locations: ChartExGeoLocations => "geoLocations" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoLocationQueryResults {
    pub geo_location_query_result: Vec<ChartExGeoLocationQueryResult>,
}
native_element!(
    ChartExGeoLocationQueryResults,
    attrs {
    },
    children {
        geo_location_query_result: ChartExGeoLocationQueryResult => "geoLocationQueryResult" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoPolygon {
    pub polygon_id: String,
    pub num_points: ChartExInteger,
    pub pca_rings:  String,
}
native_element!(
    ChartExGeoPolygon,
    attrs {
        polygon_id: String => "polygonId" required,
        num_points: ChartExInteger => "numPoints" required,
        pca_rings: String => "pcaRings" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoPolygons {
    pub geo_polygon: Vec<ChartExGeoPolygon>,
}
native_element!(
    ChartExGeoPolygons,
    attrs {
    },
    children {
        geo_polygon: ChartExGeoPolygon => "geoPolygon" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExCopyrights {
    pub copyright: Vec<String>,
}
native_element!(
    ChartExCopyrights,
    attrs {
    },
    children {
        copyright: String => "copyright" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoData {
    pub entity_name:  String,
    pub entity_id:    String,
    pub east:         XmlDouble,
    pub west:         XmlDouble,
    pub north:        XmlDouble,
    pub south:        XmlDouble,
    pub geo_polygons: Option<ChartExGeoPolygons>,
    pub copyrights:   Option<ChartExCopyrights>,
}
native_element!(
    ChartExGeoData,
    attrs {
        entity_name: String => "entityName" required,
        entity_id: String => "entityId" required,
        east: XmlDouble => "east" required,
        west: XmlDouble => "west" required,
        north: XmlDouble => "north" required,
        south: XmlDouble => "south" required,
    },
    children {
        geo_polygons: ChartExGeoPolygons => "geoPolygons" one,
        copyrights: ChartExCopyrights => "copyrights" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataEntityQuery {
    pub entity_type: ChartExEntityType,
    pub entity_id:   String,
}
native_element!(
    ChartExGeoDataEntityQuery,
    attrs {
        entity_type: ChartExEntityType => "entityType" required,
        entity_id: String => "entityId" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataEntityQueryResult {
    pub geo_data_entity_query: Option<ChartExGeoDataEntityQuery>,
    pub geo_data:              Option<ChartExGeoData>,
}
native_element!(
    ChartExGeoDataEntityQueryResult,
    attrs {
    },
    children {
        geo_data_entity_query: ChartExGeoDataEntityQuery => "geoDataEntityQuery" one,
        geo_data: ChartExGeoData => "geoData" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataEntityQueryResults {
    pub geo_data_entity_query_result: Vec<ChartExGeoDataEntityQueryResult>,
}
native_element!(
    ChartExGeoDataEntityQueryResults,
    attrs {
    },
    children {
        geo_data_entity_query_result: ChartExGeoDataEntityQueryResult => "geoDataEntityQueryResult" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataPointQuery {
    pub entity_type: ChartExEntityType,
    pub latitude:    XmlDouble,
    pub longitude:   XmlDouble,
}
native_element!(
    ChartExGeoDataPointQuery,
    attrs {
        entity_type: ChartExEntityType => "entityType" required,
        latitude: XmlDouble => "latitude" required,
        longitude: XmlDouble => "longitude" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataPointToEntityQuery {
    pub entity_type: ChartExEntityType,
    pub entity_id:   String,
}
native_element!(
    ChartExGeoDataPointToEntityQuery,
    attrs {
        entity_type: ChartExEntityType => "entityType" required,
        entity_id: String => "entityId" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataPointToEntityQueryResult {
    pub geo_data_point_query:           Option<ChartExGeoDataPointQuery>,
    pub geo_data_point_to_entity_query: Option<ChartExGeoDataPointToEntityQuery>,
}
native_element!(
    ChartExGeoDataPointToEntityQueryResult,
    attrs {
    },
    children {
        geo_data_point_query: ChartExGeoDataPointQuery => "geoDataPointQuery" one,
        geo_data_point_to_entity_query: ChartExGeoDataPointToEntityQuery => "geoDataPointToEntityQuery" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoDataPointToEntityQueryResults {
    pub geo_data_point_to_entity_query_result: Vec<ChartExGeoDataPointToEntityQueryResult>,
}
native_element!(
    ChartExGeoDataPointToEntityQueryResults,
    attrs {
    },
    children {
        geo_data_point_to_entity_query_result: ChartExGeoDataPointToEntityQueryResult => "geoDataPointToEntityQueryResult" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoChildTypes {
    pub entity_type: Vec<ChartExEntityType>,
}
native_element!(
    ChartExGeoChildTypes,
    attrs {
    },
    children {
        entity_type: ChartExEntityType => "entityType" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoChildEntitiesQuery {
    pub entity_id:       String,
    pub geo_child_types: Option<ChartExGeoChildTypes>,
}
native_element!(
    ChartExGeoChildEntitiesQuery,
    attrs {
        entity_id: String => "entityId" required,
    },
    children {
        geo_child_types: ChartExGeoChildTypes => "geoChildTypes" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoHierarchyEntity {
    pub entity_name: String,
    pub entity_id:   String,
    pub entity_type: ChartExEntityType,
}
native_element!(
    ChartExGeoHierarchyEntity,
    attrs {
        entity_name: String => "entityName" required,
        entity_id: String => "entityId" required,
        entity_type: ChartExEntityType => "entityType" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoChildEntities {
    pub geo_hierarchy_entity: Vec<ChartExGeoHierarchyEntity>,
}
native_element!(
    ChartExGeoChildEntities,
    attrs {
    },
    children {
        geo_hierarchy_entity: ChartExGeoHierarchyEntity => "geoHierarchyEntity" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoChildEntitiesQueryResult {
    pub geo_child_entities_query: Option<ChartExGeoChildEntitiesQuery>,
    pub geo_child_entities:       Option<ChartExGeoChildEntities>,
}
native_element!(
    ChartExGeoChildEntitiesQueryResult,
    attrs {
    },
    children {
        geo_child_entities_query: ChartExGeoChildEntitiesQuery => "geoChildEntitiesQuery" one,
        geo_child_entities: ChartExGeoChildEntities => "geoChildEntities" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoChildEntitiesQueryResults {
    pub geo_child_entities_query_result: Vec<ChartExGeoChildEntitiesQueryResult>,
}
native_element!(
    ChartExGeoChildEntitiesQueryResults,
    attrs {
    },
    children {
        geo_child_entities_query_result: ChartExGeoChildEntitiesQueryResult => "geoChildEntitiesQueryResult" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoParentEntitiesQuery {
    pub entity_id: String,
}
native_element!(
    ChartExGeoParentEntitiesQuery,
    attrs {
        entity_id: String => "entityId" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoEntity {
    pub entity_name: String,
    pub entity_type: ChartExEntityType,
}
native_element!(
    ChartExGeoEntity,
    attrs {
        entity_name: String => "entityName" required,
        entity_type: ChartExEntityType => "entityType" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoParentEntity {
    pub entity_id: String,
}
native_element!(
    ChartExGeoParentEntity,
    attrs {
        entity_id: String => "entityId" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoParentEntitiesQueryResult {
    pub geo_parent_entities_query: Option<ChartExGeoParentEntitiesQuery>,
    pub geo_entity:                Option<ChartExGeoEntity>,
    pub geo_parent_entity:         Option<ChartExGeoParentEntity>,
}
native_element!(
    ChartExGeoParentEntitiesQueryResult,
    attrs {
    },
    children {
        geo_parent_entities_query: ChartExGeoParentEntitiesQuery => "geoParentEntitiesQuery" one,
        geo_entity: ChartExGeoEntity => "geoEntity" one,
        geo_parent_entity: ChartExGeoParentEntity => "geoParentEntity" one,
    },
    required [geo_parent_entities_query],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoParentEntitiesQueryResults {
    pub geo_parent_entities_query_result: Vec<ChartExGeoParentEntitiesQueryResult>,
}
native_element!(
    ChartExGeoParentEntitiesQueryResults,
    attrs {
    },
    children {
        geo_parent_entities_query_result: ChartExGeoParentEntitiesQueryResult => "geoParentEntitiesQueryResult" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExClear {
    pub geo_location_query_results:             Option<ChartExGeoLocationQueryResults>,
    pub geo_data_entity_query_results:          Option<ChartExGeoDataEntityQueryResults>,
    pub geo_data_point_to_entity_query_results: Option<ChartExGeoDataPointToEntityQueryResults>,
    pub geo_child_entities_query_results:       Option<ChartExGeoChildEntitiesQueryResults>,
    pub geo_parent_entities_query_results:      Option<ChartExGeoParentEntitiesQueryResults>,
}
native_element!(
    ChartExClear,
    attrs {
    },
    children {
        geo_location_query_results: ChartExGeoLocationQueryResults => "geoLocationQueryResults" one,
        geo_data_entity_query_results: ChartExGeoDataEntityQueryResults => "geoDataEntityQueryResults" one,
        geo_data_point_to_entity_query_results: ChartExGeoDataPointToEntityQueryResults => "geoDataPointToEntityQueryResults" one,
        geo_child_entities_query_results: ChartExGeoChildEntitiesQueryResults => "geoChildEntitiesQueryResults" one,
        geo_parent_entities_query_results: ChartExGeoParentEntitiesQueryResults => "geoParentEntitiesQueryResults" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeoCache {
    pub provider: String,
    pub entries:  Vec<ChartExGeoCacheEntry>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ChartExGeoCacheEntry {
    Binary(ChartExBinary),
    Clear(ChartExClear),
}
impl NativeElement for ChartExGeoCache {
    fn read<R: BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        parent: &NamespaceScope,
        session: &SourceCacheSession,
    ) -> Result<Self, String> {
        let scope = parent.child(element)?;
        let _guard = session.push_extension_scope(scope.clone());
        let mut attrs = Attributes::read(element, &scope)?;
        let provider = attrs.required("provider")?;
        attrs.finish()?;
        let mut entries = Vec::new();
        children(
            reader,
            element,
            empty,
            &scope,
            |reader, child, empty, name| {
                entries.push(match name {
                    "binary" => ChartExGeoCacheEntry::Binary(ChartExBinary::read(
                        reader, child, empty, &scope, session,
                    )?),
                    "clear" => ChartExGeoCacheEntry::Clear(ChartExClear::read(
                        reader, child, empty, &scope, session,
                    )?),
                    _ => return Err(format!("unsupported ChartEx geo-cache child {name}")),
                });
                Ok(())
            },
        )?;
        Ok(Self { provider, entries })
    }

    fn write(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
        context: &mut WriteContext<'_>,
    ) -> Result<(), String> {
        start(
            writer,
            name,
            vec![("provider", self.provider.clone())],
            false,
        );
        for entry in &self.entries {
            match entry {
                ChartExGeoCacheEntry::Binary(value) => value.write(writer, "cx:binary", context)?,
                ChartExGeoCacheEntry::Clear(value) => value.write(writer, "cx:clear", context)?,
            }
        }
        write_end_tag(writer, name);
        Ok(())
    }

    fn visit_formulas(&self, visit: &mut dyn FnMut(&Formula)) {
        for entry in &self.entries {
            if let ChartExGeoCacheEntry::Clear(value) = entry {
                value.visit_formulas(visit);
            }
        }
    }

    fn visit_formulas_mut(&mut self, visit: &mut dyn FnMut(&mut Formula)) {
        for entry in &mut self.entries {
            if let ChartExGeoCacheEntry::Clear(value) = entry {
                value.visit_formulas_mut(visit);
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExGeography {
    pub projection_type:    Option<ChartExGeoProjectionType>,
    pub viewed_region_type: Option<ChartExGeoMappingLevel>,
    pub culture_language:   String,
    pub culture_region:     String,
    pub attribution:        String,
    pub geo_cache:          Option<ChartExGeoCache>,
}
native_element!(
    ChartExGeography,
    attrs {
        projection_type: ChartExGeoProjectionType => "projectionType" optional,
        viewed_region_type: ChartExGeoMappingLevel => "viewedRegionType" optional,
        culture_language: String => "cultureLanguage" required,
        culture_region: String => "cultureRegion" required,
        attribution: String => "attribution" required,
    },
    children {
        geo_cache: ChartExGeoCache => "geoCache" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExStatistics {
    pub quartile_method: Option<ChartExQuartileMethod>,
}
native_element!(
    ChartExStatistics,
    attrs {
        quartile_method: ChartExQuartileMethod => "quartileMethod" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExSubtotalIndex {
    pub value: u32,
}
native_element!(
    ChartExSubtotalIndex,
    attrs {
        value: u32 => "val" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExSubtotals {
    pub indices: Vec<ChartExSubtotalIndex>,
}
native_element!(
    ChartExSubtotals,
    attrs {
    },
    children {
        indices: ChartExSubtotalIndex => "idx" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExLayoutProperties {
    pub parent_label_layout: Option<ChartExParentLabelProperties>,
    pub region_label_layout: Option<ChartExRegionLabelProperties>,
    pub visibility:          Option<ChartExSeriesElementVisibilities>,
    pub aggregation:         Option<ChartExAggregation>,
    pub binning:             Option<ChartExBinning>,
    pub geography:           Option<ChartExGeography>,
    pub statistics:          Option<ChartExStatistics>,
    pub subtotals:           Option<ChartExSubtotals>,
    pub extensions:          Option<ChartExExtensionList>,
}
native_element!(
    ChartExLayoutProperties,
    attrs {
    },
    children {
        parent_label_layout: ChartExParentLabelProperties => "parentLabelLayout" one,
        region_label_layout: ChartExRegionLabelProperties => "regionLabelLayout" one,
        visibility: ChartExSeriesElementVisibilities => "visibility" one,
        aggregation: ChartExAggregation => "aggregation" one,
        binning: ChartExBinning => "binning" one,
        geography: ChartExGeography => "geography" one,
        statistics: ChartExStatistics => "statistics" one,
        subtotals: ChartExSubtotals => "subtotals" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices [optional [aggregation,binning]]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataPoint {
    pub index:            u32,
    pub shape_properties: Option<ShapeProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExDataPoint,
    attrs {
        index: u32 => "idx" required,
    },
    children {
        shape_properties: ShapeProperties => "spPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExExtremeValueColorPosition {}
native_element!(
    ChartExExtremeValueColorPosition,
    attrs {
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExNumberColorPosition {
    pub value: XmlDouble,
}
native_element!(
    ChartExNumberColorPosition,
    attrs {
        value: XmlDouble => "val" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPercentageColorPosition {
    pub value: XmlDouble,
}
native_element!(
    ChartExPercentageColorPosition,
    attrs {
        value: XmlDouble => "val" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExValueColorEndPosition {
    pub extreme_value: Option<ChartExExtremeValueColorPosition>,
    pub number:        Option<ChartExNumberColorPosition>,
    pub percent:       Option<ChartExPercentageColorPosition>,
}
native_element!(
    ChartExValueColorEndPosition,
    attrs {
    },
    children {
        extreme_value: ChartExExtremeValueColorPosition => "extremeValue" one,
        number: ChartExNumberColorPosition => "number" one,
        percent: ChartExPercentageColorPosition => "percent" one,
    },
    required [],
    choices [required [extreme_value,number,percent]]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExValueColorMiddlePosition {
    pub number:  Option<ChartExNumberColorPosition>,
    pub percent: Option<ChartExPercentageColorPosition>,
}
native_element!(
    ChartExValueColorMiddlePosition,
    attrs {
    },
    children {
        number: ChartExNumberColorPosition => "number" one,
        percent: ChartExPercentageColorPosition => "percent" one,
    },
    required [],
    choices [required [number, percent]]
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExValueColorPositions {
    pub count: Option<ChartExColorPositionCount>,
    pub min:   Option<ChartExValueColorEndPosition>,
    pub mid:   Option<ChartExValueColorMiddlePosition>,
    pub max:   Option<ChartExValueColorEndPosition>,
}
native_element!(
    ChartExValueColorPositions,
    attrs {
        count: ChartExColorPositionCount => "count" optional,
    },
    children {
        min: ChartExValueColorEndPosition => "min" one,
        mid: ChartExValueColorMiddlePosition => "mid" one,
        max: ChartExValueColorEndPosition => "max" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataLabelVisibilities {
    pub series_name:   Option<bool>,
    pub category_name: Option<bool>,
    pub value:         Option<bool>,
}
native_element!(
    ChartExDataLabelVisibilities,
    attrs {
        series_name: bool => "seriesName" optional,
        category_name: bool => "categoryName" optional,
        value: bool => "value" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataLabel {
    pub index:            u32,
    pub position:         Option<ChartExDataLabelPos>,
    pub number_format:    Option<ChartExNumberFormat>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub visibility:       Option<ChartExDataLabelVisibilities>,
    pub separator:        Option<String>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExDataLabel,
    attrs {
        index: u32 => "idx" required,
        position: ChartExDataLabelPos => "pos" optional,
    },
    children {
        number_format: ChartExNumberFormat => "numFmt" one,
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        visibility: ChartExDataLabelVisibilities => "visibility" one,
        separator: String => "separator" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataLabelHidden {
    pub index: u32,
}
native_element!(
    ChartExDataLabelHidden,
    attrs {
        index: u32 => "idx" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDataLabels {
    pub position:          Option<ChartExDataLabelPos>,
    pub number_format:     Option<ChartExNumberFormat>,
    pub shape_properties:  Option<ShapeProperties>,
    pub text_properties:   Option<TextProperties>,
    pub visibility:        Option<ChartExDataLabelVisibilities>,
    pub separator:         Option<String>,
    pub data_label:        Vec<ChartExDataLabel>,
    pub data_label_hidden: Vec<ChartExDataLabelHidden>,
    pub extensions:        Option<ChartExExtensionList>,
}
native_element!(
    ChartExDataLabels,
    attrs {
        position: ChartExDataLabelPos => "pos" optional,
    },
    children {
        number_format: ChartExNumberFormat => "numFmt" one,
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        visibility: ChartExDataLabelVisibilities => "visibility" one,
        separator: String => "separator" one,
        data_label: ChartExDataLabel => "dataLabel" many,
        data_label_hidden: ChartExDataLabelHidden => "dataLabelHidden" many,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExSeries {
    pub layout_id:             ChartExSeriesLayout,
    pub hidden:                Option<bool>,
    pub owner_idx:             Option<u32>,
    pub unique_id:             Option<String>,
    pub format_idx:            Option<u32>,
    pub text:                  Option<ChartExText>,
    pub shape_properties:      Option<ShapeProperties>,
    pub value_colors:          Option<ChartExValueColors>,
    pub value_color_positions: Option<ChartExValueColorPositions>,
    pub data_points:           Vec<ChartExDataPoint>,
    pub data_labels:           Option<ChartExDataLabels>,
    pub data_id:               Option<ChartExDataId>,
    pub layout_properties:     Option<ChartExLayoutProperties>,
    pub axis_ids:              Vec<u32>,
    pub extensions:            Option<ChartExExtensionList>,
}
native_element!(
    ChartExSeries,
    attrs {
        layout_id: ChartExSeriesLayout => "layoutId" required,
        hidden: bool => "hidden" optional,
        owner_idx: u32 => "ownerIdx" optional,
        unique_id: String => "uniqueId" optional,
        format_idx: u32 => "formatIdx" optional,
    },
    children {
        text: ChartExText => "tx" one,
        shape_properties: ShapeProperties => "spPr" one,
        value_colors: ChartExValueColors => "valueColors" one,
        value_color_positions: ChartExValueColorPositions => "valueColorPositions" one,
        data_points: ChartExDataPoint => "dataPt" many,
        data_labels: ChartExDataLabels => "dataLabels" one,
        data_id: ChartExDataId => "dataId" one,
        layout_properties: ChartExLayoutProperties => "layoutPr" one,
        axis_ids: u32 => "axisId" many,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExLegend {
    pub position:         Option<ChartExSidePos>,
    pub align:            Option<ChartExPosAlign>,
    pub overlay:          Option<bool>,
    pub shape_properties: Option<ShapeProperties>,
    pub text_properties:  Option<TextProperties>,
    pub offset:           Option<ChartExOffset>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExLegend,
    attrs {
        position: ChartExSidePos => "pos" optional,
        align: ChartExPosAlign => "align" optional,
        overlay: bool => "overlay" optional,
    },
    children {
        shape_properties: ShapeProperties => "spPr" one,
        text_properties: TextProperties => "txPr" one,
        offset: ChartExOffset => "offset" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPlotSurface {
    pub shape_properties: Option<ShapeProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExPlotSurface,
    attrs {
    },
    children {
        shape_properties: ShapeProperties => "spPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPlotAreaRegion {
    pub plot_surface: Option<ChartExPlotSurface>,
    pub series:       Vec<ChartExSeries>,
    pub extensions:   Option<ChartExExtensionList>,
}
native_element!(
    ChartExPlotAreaRegion,
    attrs {
    },
    children {
        plot_surface: ChartExPlotSurface => "plotSurface" one,
        series: ChartExSeries => "series" many,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPlotArea {
    pub plot_area_region: Option<ChartExPlotAreaRegion>,
    pub axis:             Vec<ChartExAxis>,
    pub shape_properties: Option<ShapeProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExPlotArea,
    attrs {
    },
    children {
        plot_area_region: ChartExPlotAreaRegion => "plotAreaRegion" one,
        axis: ChartExAxis => "axis" many,
        shape_properties: ShapeProperties => "spPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [plot_area_region],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExChart {
    pub title:      Option<ChartExChartTitle>,
    pub plot_area:  Option<ChartExPlotArea>,
    pub legend:     Option<ChartExLegend>,
    pub extensions: Option<ChartExExtensionList>,
}
native_element!(
    ChartExChart,
    attrs {
    },
    children {
        title: ChartExChartTitle => "title" one,
        plot_area: ChartExPlotArea => "plotArea" one,
        legend: ChartExLegend => "legend" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [plot_area],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExBinding {
    pub name: String,
    pub id:   u32,
    pub kind: ChartExBindingType,
}
native_element!(
    ChartExBinding,
    attrs {
        name: String => "name" required,
        id: u32 => "id" required,
        kind: ChartExBindingType => "type" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExBindings {
    pub binding:    Vec<ChartExBinding>,
    pub extensions: Option<ChartExExtensionList>,
}
native_element!(
    ChartExBindings,
    attrs {
    },
    children {
        binding: ChartExBinding => "binding" many,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExArtifact {
    pub source: Option<String>,
    pub name:   String,
    pub kind:   ChartExArtifactType,
}
native_element!(
    ChartExArtifact,
    attrs {
        source: String => "source" optional,
        name: String => "name" required,
        kind: ChartExArtifactType => "type" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExArtifacts {
    pub artifact:   Vec<ChartExArtifact>,
    pub extensions: Option<ChartExExtensionList>,
}
native_element!(
    ChartExArtifacts,
    attrs {
    },
    children {
        artifact: ChartExArtifact => "artifact" many,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [artifact],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExDashboard {
    pub entrypoint: String,
    pub bindings:   Option<ChartExBindings>,
    pub artifacts:  Option<ChartExArtifacts>,
    pub extensions: Option<ChartExExtensionList>,
}
native_element!(
    ChartExDashboard,
    attrs {
        entrypoint: String => "entrypoint" required,
    },
    children {
        bindings: ChartExBindings => "bindings" one,
        artifacts: ChartExArtifacts => "artifacts" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [bindings,artifacts],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExFormatOverride {
    pub index:            u32,
    pub shape_properties: Option<ShapeProperties>,
    pub extensions:       Option<ChartExExtensionList>,
}
native_element!(
    ChartExFormatOverride,
    attrs {
        index: u32 => "idx" required,
    },
    children {
        shape_properties: ShapeProperties => "spPr" one,
        extensions: ChartExExtensionList => "extLst" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExFormatOverrides {
    pub overrides: Vec<ChartExFormatOverride>,
}
native_element!(
    ChartExFormatOverrides,
    attrs {
    },
    children {
        overrides: ChartExFormatOverride => "fmtOvr" many,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExHeaderFooter {
    pub align_with_margins: Option<bool>,
    pub different_odd_even: Option<bool>,
    pub different_first:    Option<bool>,
    pub odd_header:         Option<String>,
    pub odd_footer:         Option<String>,
    pub even_header:        Option<String>,
    pub even_footer:        Option<String>,
    pub first_header:       Option<String>,
    pub first_footer:       Option<String>,
}
native_element!(
    ChartExHeaderFooter,
    attrs {
        align_with_margins: bool => "alignWithMargins" optional,
        different_odd_even: bool => "differentOddEven" optional,
        different_first: bool => "differentFirst" optional,
    },
    children {
        odd_header: String => "oddHeader" one,
        odd_footer: String => "oddFooter" one,
        even_header: String => "evenHeader" one,
        even_footer: String => "evenFooter" one,
        first_header: String => "firstHeader" one,
        first_footer: String => "firstFooter" one,
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPageMargins {
    pub l:      XmlDouble,
    pub r:      XmlDouble,
    pub t:      XmlDouble,
    pub b:      XmlDouble,
    pub header: XmlDouble,
    pub footer: XmlDouble,
}
native_element!(
    ChartExPageMargins,
    attrs {
        l: XmlDouble => "l" required,
        r: XmlDouble => "r" required,
        t: XmlDouble => "t" required,
        b: XmlDouble => "b" required,
        header: XmlDouble => "header" required,
        footer: XmlDouble => "footer" required,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPageSetup {
    pub paper_size:            Option<u32>,
    pub first_page_number:     Option<u32>,
    pub orientation:           Option<ChartExPageOrientation>,
    pub black_and_white:       Option<bool>,
    pub draft:                 Option<bool>,
    pub use_first_page_number: Option<bool>,
    pub horizontal_dpi:        Option<i32>,
    pub vertical_dpi:          Option<i32>,
    pub copies:                Option<u32>,
}
native_element!(
    ChartExPageSetup,
    attrs {
        paper_size: u32 => "paperSize" optional,
        first_page_number: u32 => "firstPageNumber" optional,
        orientation: ChartExPageOrientation => "orientation" optional,
        black_and_white: bool => "blackAndWhite" optional,
        draft: bool => "draft" optional,
        use_first_page_number: bool => "useFirstPageNumber" optional,
        horizontal_dpi: i32 => "horizontalDpi" optional,
        vertical_dpi: i32 => "verticalDpi" optional,
        copies: u32 => "copies" optional,
    },
    children {
    },
    required [],
    choices []
);

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartExPrintSettings {
    pub header_footer: Option<ChartExHeaderFooter>,
    pub page_margins:  Option<ChartExPageMargins>,
    pub page_setup:    Option<ChartExPageSetup>,
}
native_element!(
    ChartExPrintSettings,
    attrs {
    },
    children {
        header_footer: ChartExHeaderFooter => "headerFooter" one,
        page_margins: ChartExPageMargins => "pageMargins" one,
        page_setup: ChartExPageSetup => "pageSetup" one,
    },
    required [],
    choices []
);
