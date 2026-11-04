// c:bubbleSize
use super::numeric_data::numeric_source;

numeric_source!(
    /// `c:bubbleSize`, a numeric data source holding either a reference or, for
    /// a series typed as an array constant such as `={1,2,3}`, a literal.
    /// The literal is the source while it is present.
    BubbleSize,
    "bubbleSize",
    "bubble sizes"
);
