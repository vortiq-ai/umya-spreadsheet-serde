// c:yVal
use super::numeric_data::numeric_source;

numeric_source!(
    /// `c:yVal`, a numeric data source holding either a reference or, for a
    /// series typed as an array constant such as `={1,2,3}`, a literal. The
    /// literal is the source while it is present.
    YValues,
    "yVal",
    "y values"
);
