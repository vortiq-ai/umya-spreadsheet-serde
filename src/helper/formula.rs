use crate::{
    helper::{
        address::{
            join_address,
            split_address,
            unescape_sheet_name,
        },
        coordinate::{
            adjustment_insert_coordinate,
            adjustment_remove_coordinate,
            coordinate_from_index_with_lock,
            index_from_coordinate,
            string_from_column_index,
        },
        range::{
            get_join_range,
            get_split_range,
        },
        utils::compile_regex,
    },
    structs::StringValue,
};

/// PARTLY BASED ON: */
/// Copyright (c) 2007 E. W. Bachtal, Inc. */
/// <https://ewbi.blogs.com/develops/2007/03/excel_formula_p.html> */
/// <https://ewbi.blogs.com/develops/2004/12/excel_formula_p.html>

#[derive(Clone, Debug, PartialEq)]
pub enum FormulaTokenTypes {
    Noop,
    Operand,
    Function,
    Subexpression,
    Argument,
    OperatorPrefix,
    OperatorInfix,
    OperatorPostfix,
    Whitespace,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FormulaTokenSubTypes {
    Nothing,
    Start,
    Stop,
    Text,
    Number,
    Logical,
    Error,
    Range,
    Math,
    Concatenation,
    Intersection,
    Union,
}

#[derive(Clone, Debug)]
pub struct FormulaToken {
    value:          StringValue,
    token_type:     FormulaTokenTypes,
    token_sub_type: FormulaTokenSubTypes,
    // Byte offsets in the original tokenizer input, when this token was
    // emitted directly from an unchanged operand. Synthetic tokens have none.
    source_span:    Option<std::ops::Range<usize>>,
}
impl Default for FormulaToken {
    #[inline]
    fn default() -> Self {
        Self {
            value:          StringValue::default(),
            token_type:     FormulaTokenTypes::Unknown,
            token_sub_type: FormulaTokenSubTypes::Nothing,
            source_span:    None,
        }
    }
}
impl FormulaToken {
    pub(crate) fn source_span(&self) -> Option<std::ops::Range<usize>> {
        self.source_span.clone()
    }

    #[inline]
    #[must_use]
    pub fn get_value(&self) -> &str {
        self.value.value_str()
    }

    #[inline]
    pub fn set_value<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.value.set_value(value);
        self.source_span = None;
        self
    }

    #[inline]
    #[must_use]
    pub fn get_token_type(&self) -> &FormulaTokenTypes {
        &self.token_type
    }

    #[inline]
    pub fn set_token_type(&mut self, value: FormulaTokenTypes) -> &mut Self {
        self.token_type = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn get_token_sub_type(&self) -> &FormulaTokenSubTypes {
        &self.token_sub_type
    }

    #[inline]
    pub fn set_token_sub_type(&mut self, value: FormulaTokenSubTypes) -> &mut Self {
        self.token_sub_type = value;
        self
    }
}

struct ChartFormulaCandidate {
    token: FormulaToken,
    span:  std::ops::Range<usize>,
}

/// A genuine source-backed reference/name operand, never synthetic syntax.
pub(crate) struct ChartFormulaOperand<'a> {
    pub(crate) span: std::ops::Range<usize>,
    pub(crate) text: &'a str,
}

fn annotate_operand_spans(
    offsets: Option<&[usize]>,
    end_index: usize,
    tokens: &mut [FormulaToken],
    candidates: &mut Vec<ChartFormulaCandidate>,
    collect_operands: bool,
) {
    let Some(offsets) = offsets else {
        return;
    };
    for token in tokens {
        if token.token_type != FormulaTokenTypes::Operand
            || token.token_sub_type == FormulaTokenSubTypes::Text
        {
            continue;
        }
        let value = token.get_value();
        if value.is_empty() {
            continue;
        }
        // Nontext operands still contain the exact raw bytes accumulated by
        // the lexer. Delimiters flush BEFORE their own character; a completed
        // error flushes AFTER its last character. The caller supplies that
        // actual emission boundary, rather than guessing/failing a span.
        let end = offsets[end_index];
        let span = end - value.len()..end;
        token.source_span = Some(span.clone());
        if collect_operands {
            candidates.push(ChartFormulaCandidate {
                token: token.clone(),
                span,
            });
        }
    }
}

const QUOTE_DOUBLE: char = '"';
const QUOTE_SINGLE: char = '\'';
const BRACKET_CLOSE: char = ']';
const BRACKET_OPEN: char = '[';
const BRACE_OPEN: char = '{';
const BRACE_CLOSE: char = '}';
const PAREN_OPEN: char = '(';
const PAREN_CLOSE: char = ')';
const SEMICOLON: char = ';';
const WHITESPACE: char = ' ';
const COMMA: char = ',';
const ERROR_START: char = '#';

const OPERATORS_SN: &str = "+-";
const OPERATORS_INFIX: &str = "+-*/^&=><";
const OPERATORS_POSTFIX: &str = "%";

pub const ERRORS: &[&str] = &[
    "#NULL!", "#DIV/0!", "#VALUE!", "#REF!", "#NAME?", "#NUM!", "#N/A",
];
const COMPARATORS_MULTI: &[&str] = &[">=", "<=", "<>"];

macro_rules! token {
    ($value:expr, $typ:expr, $sub:expr) => {{
        let mut obj = FormulaToken::default();
        obj.set_value($value);
        obj.set_token_type($typ);
        obj.set_token_sub_type($sub);
        obj
    }};
}

pub(crate) fn parse_to_tokens<S: Into<String>>(formula: S) -> Vec<FormulaToken> {
    let formula_str = formula.into();
    parse_tokens_inner(&formula_str, false)
}

fn parse_tokens_inner(formula_str: &str, capture_spans: bool) -> Vec<FormulaToken> {
    let formula_length = formula_str.chars().count();

    // quick checks
    if formula_length < 2 || !formula_str.starts_with('=') {
        return Vec::new();
    }

    let (_, tokens2, _) = parse_into_intermediate_tokens(formula_str, capture_spans, false);
    finalize_tokens(&tokens2)
}

/// Tokenize chart formula text with the existing lexer. Chart XML commonly
/// omits the leading `=`; returned operand spans always index the input, not
/// the temporary prefix supplied to the lexer.
pub(crate) fn parse_chart_formula_tokens(formula: &str) -> Vec<FormulaToken> {
    if formula.starts_with('=') {
        return parse_tokens_inner(formula, true);
    }
    let mut tokens = parse_tokens_inner(&format!("={formula}"), true);
    for token in &mut tokens {
        if let Some(span) = &mut token.source_span {
            // Every raw operand begins after the synthetic leading '='.
            span.start -= 1;
            span.end -= 1;
        }
    }
    tokens
}

/// Project mandatory raw spans emitted by the SAME lexer. Reference
/// classification reuses its ordinary token finalization; no Option/Result
/// conversion can discard an otherwise editable reference with a missing span.
pub(crate) fn chart_formula_operands(input: &str) -> Vec<ChartFormulaOperand<'_>> {
    let prefixed;
    let (formula, prefix_bytes) = if input.starts_with('=') {
        (input, 0)
    } else {
        prefixed = format!("={input}");
        (prefixed.as_str(), 1)
    };
    let (_, _, candidates) = parse_into_intermediate_tokens(formula, true, true);
    let mut operands = Vec::new();
    for candidate in candidates {
        for token in finalize_tokens(std::slice::from_ref(&candidate.token)) {
            if token.get_token_type() == &FormulaTokenTypes::Operand
                && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
            {
                let span = candidate.span.start - prefix_bytes..candidate.span.end - prefix_bytes;
                operands.push(ChartFormulaOperand {
                    text: &input[span.clone()],
                    span,
                });
            }
        }
    }
    operands
}

fn parse_into_intermediate_tokens(
    formula: &str,
    capture_spans: bool,
    collect_operands: bool,
) -> (
    Vec<FormulaToken>,
    Vec<FormulaToken>,
    Vec<ChartFormulaCandidate>,
) {
    let mut tokens1: Vec<FormulaToken> = Vec::new();
    let mut tokens2: Vec<FormulaToken> = Vec::new();
    let mut stack: Vec<FormulaToken> = Vec::new();
    let mut candidates = Vec::new();

    // state flags
    let mut in_string = false;
    let mut in_path = false;
    let mut range_depth = 0;
    let mut in_error = false;

    let mut index = 1;
    let mut value = String::new();
    let formula_length = formula.chars().count();
    let offsets = capture_spans.then(|| {
        let mut offsets: Vec<usize> = formula.char_indices().map(|(byte, _)| byte).collect();
        offsets.push(formula.len());
        offsets
    });

    while index < formula_length {
        let before = index;
        let first = tokens1.len();
        // double-quoted strings
        if in_string {
            handle_in_string(
                formula,
                &mut index,
                &mut value,
                &mut tokens1,
                &mut in_string,
            );
            annotate_operand_spans(
                offsets.as_deref(),
                before,
                &mut tokens1[first..],
                &mut candidates,
                collect_operands,
            );
            continue;
        }

        // single-quoted strings (links)
        if in_path {
            handle_in_path(formula, &mut index, &mut value, &mut in_path);
            annotate_operand_spans(
                offsets.as_deref(),
                before,
                &mut tokens1[first..],
                &mut candidates,
                collect_operands,
            );
            continue;
        }

        // bracketed strings (R1C1 range index or workbook name)
        if range_depth > 0 {
            handle_in_range(formula, &mut index, &mut value, &mut range_depth);
            annotate_operand_spans(
                offsets.as_deref(),
                before,
                &mut tokens1[first..],
                &mut candidates,
                collect_operands,
            );
            continue;
        }

        // error values
        if in_error {
            handle_in_error(formula, &mut index, &mut value, &mut tokens1, &mut in_error);
            annotate_operand_spans(
                offsets.as_deref(),
                index,
                &mut tokens1[first..],
                &mut candidates,
                collect_operands,
            );
            continue;
        }

        // scientific notation check
        if handle_scientific_notation(formula, &mut index, &mut value) {
            annotate_operand_spans(
                offsets.as_deref(),
                before,
                &mut tokens1[first..],
                &mut candidates,
                collect_operands,
            );
            continue;
        }

        // handle special characters
        if handle_special_characters(
            formula,
            &mut index,
            &mut value,
            &mut tokens1,
            &mut stack,
            &mut in_string,
            &mut range_depth,
            &mut in_path,
            &mut in_error,
        ) {
            annotate_operand_spans(
                offsets.as_deref(),
                before,
                &mut tokens1[first..],
                &mut candidates,
                collect_operands,
            );
            continue;
        }

        // accumulate as normal char
        value.push(formula.chars().nth(index).unwrap());
        index += 1;
        annotate_operand_spans(
            offsets.as_deref(),
            before,
            &mut tokens1[first..],
            &mut candidates,
            collect_operands,
        );
    }

    // dump remaining
    if !value.is_empty() {
        let mut obj = FormulaToken::default();
        obj.set_value(value.clone());
        obj.set_token_type(if in_string || in_path || range_depth > 0 || in_error {
            // An unfinished lexical owner is preserved malformed syntax, not
            // an editable reference merely because its trailing bytes look
            // like a cell address.
            FormulaTokenTypes::Unknown
        } else {
            FormulaTokenTypes::Operand
        });
        annotate_operand_spans(
            offsets.as_deref(),
            index,
            std::slice::from_mut(&mut obj),
            &mut candidates,
            collect_operands,
        );
        tokens1.push(obj);
    }

    // trim whitespace, handle intersection
    cleanup_tokens(&tokens1, &mut tokens2);

    (tokens1, tokens2, candidates)
}

fn finalize_tokens(tokens_in: &[FormulaToken]) -> Vec<FormulaToken> {
    let mut tokens: Vec<FormulaToken> = Vec::new();
    let token_count = tokens_in.len();
    let mut previous_token: Option<FormulaToken>;

    for i in 0..token_count {
        let mut token = tokens_in[i].clone();
        if i > 0 {
            previous_token = Some(tokens_in[i - 1].clone());
        } else {
            previous_token = None;
        }

        // switch infix "-" to prefix
        if token.get_token_type() == &FormulaTokenTypes::OperatorInfix && token.get_value() == "-" {
            if i == 0 {
                token.set_token_type(FormulaTokenTypes::OperatorPrefix);
            } else if should_token_be_math(previous_token) {
                token.set_token_sub_type(FormulaTokenSubTypes::Math);
            } else {
                token.set_token_type(FormulaTokenTypes::OperatorPrefix);
            }
            tokens.push(token.clone());
            continue;
        }

        // switch infix "+" to prefix/noop
        if token.get_token_type() == &FormulaTokenTypes::OperatorInfix && token.get_value() == "+" {
            if i == 0 {
                // skip
                continue;
            } else if should_token_be_math(previous_token) {
                token.set_token_sub_type(FormulaTokenSubTypes::Math);
            } else {
                // skip
                continue;
            }
            tokens.push(token.clone());
            continue;
        }

        // set operator subtypes
        if token.get_token_type() == &FormulaTokenTypes::OperatorInfix
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Nothing
        {
            if let Some(c) = token.get_value().chars().next() {
                if "<>=".contains(c) {
                    token.set_token_sub_type(FormulaTokenSubTypes::Logical);
                } else if c == '&' {
                    token.set_token_sub_type(FormulaTokenSubTypes::Concatenation);
                } else {
                    token.set_token_sub_type(FormulaTokenSubTypes::Math);
                }
            }
            tokens.push(token.clone());
            continue;
        }

        // set operand subtypes
        if token.get_token_type() == &FormulaTokenTypes::Operand
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Nothing
        {
            if token.get_value().parse::<f64>().is_ok() {
                token.set_token_sub_type(FormulaTokenSubTypes::Number);
            } else if ["TRUE", "FALSE"].contains(&token.get_value().to_uppercase().as_str()) {
                token.set_token_sub_type(FormulaTokenSubTypes::Logical);
            } else {
                token.set_token_sub_type(FormulaTokenSubTypes::Range);
            }
            tokens.push(token.clone());
            continue;
        }

        // remove leading '@' from function
        if let FormulaTokenTypes::Function = token.get_token_type() {
            let val = token.get_value();
            if val.starts_with('@') {
                token.set_value(val.trim_start_matches('@').to_string());
            }
        }

        tokens.push(token.clone());
    }
    tokens
}

// --------------------------------------------------------------------------
// Below are the smaller "handler" helper functions referenced above.
// --------------------------------------------------------------------------

fn handle_in_string(
    formula: &str,
    index: &mut usize,
    value: &mut String,
    tokens: &mut Vec<FormulaToken>,
    in_string: &mut bool,
) {
    if formula.chars().nth(*index).unwrap() == QUOTE_DOUBLE {
        // double quote check
        if (*index + 2) <= formula.chars().count()
            && formula.chars().nth(*index + 1).unwrap() == QUOTE_DOUBLE
        {
            *value = format!("{value}{QUOTE_DOUBLE}");
            *index += 1;
        } else {
            *in_string = false;
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            obj.set_token_sub_type(FormulaTokenSubTypes::Text);
            tokens.push(obj);
            *value = String::new();
        }
    } else {
        value.push(formula.chars().nth(*index).unwrap());
    }
    *index += 1;
}

fn handle_in_path(formula: &str, index: &mut usize, value: &mut String, in_path: &mut bool) {
    if formula.chars().nth(*index).unwrap() == QUOTE_SINGLE {
        if (*index + 2) <= formula.chars().count()
            && formula.chars().nth(*index + 1).unwrap() == QUOTE_SINGLE
        {
            // escaped quote inside the sheet name: keep both so it round-trips
            *value = format!("{value}{QUOTE_SINGLE}{QUOTE_SINGLE}");
            *index += 1;
        } else {
            // closing quote: keep it so the sheet reference stays quoted
            value.push(QUOTE_SINGLE);
            *in_path = false;
        }
    } else {
        value.push(formula.chars().nth(*index).unwrap());
    }
    *index += 1;
}

fn handle_in_range(formula: &str, index: &mut usize, value: &mut String, range_depth: &mut usize) {
    let current_char = formula.chars().nth(*index).unwrap();
    if current_char == BRACKET_OPEN {
        *range_depth += 1;
    } else if current_char == BRACKET_CLOSE {
        *range_depth = range_depth.saturating_sub(1);
    }
    value.push(current_char);
    *index += 1;
}

fn handle_in_error(
    formula: &str,
    index: &mut usize,
    value: &mut String,
    tokens: &mut Vec<FormulaToken>,
    in_error: &mut bool,
) {
    value.push(formula.chars().nth(*index).unwrap());
    *index += 1;
    // A sheet-qualified reference error is still a reference operand. Keep
    // its qualifier attached so structural edits can rename/delete the sheet
    // without treating a literal or bare error as a reference.
    let qualified_reference_error = value.ends_with("!#REF!");
    if ERRORS.contains(&value.as_str()) || qualified_reference_error {
        *in_error = false;
        let mut obj = FormulaToken::default();
        obj.set_value(value.clone());
        obj.set_token_type(FormulaTokenTypes::Operand);
        obj.set_token_sub_type(if qualified_reference_error {
            FormulaTokenSubTypes::Range
        } else {
            FormulaTokenSubTypes::Error
        });
        tokens.push(obj);
        *value = String::new();
    }
}

fn handle_scientific_notation(formula: &str, index: &mut usize, value: &mut String) -> bool {
    if let Some(current_char) = formula.chars().nth(*index) {
        if OPERATORS_SN.contains(current_char)
            && value.len() > 1
            && compile_regex!(r"/^[1-9]{1}(\\.\\d+)?E{1}$/")
                .is_match(&current_char.to_string())
                .unwrap_or(false)
        {
            value.push(current_char);
            *index += 1;
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn handle_special_characters(
    formula: &str,
    index: &mut usize,
    value: &mut String,
    tokens1: &mut Vec<FormulaToken>,
    stack: &mut Vec<FormulaToken>,
    in_string: &mut bool,
    range_depth: &mut usize,
    in_path: &mut bool,
    in_error: &mut bool,
) -> bool {
    let current_char = formula.chars().nth(*index).unwrap();

    // handle double quote
    if current_char == QUOTE_DOUBLE {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Unknown);
            tokens1.push(obj);
            value.clear();
        }
        *in_string = true;
        *index += 1;
        return true;
    }

    // handle single quote (start of a quoted sheet path, e.g. '(1)'!$E$38)
    if current_char == QUOTE_SINGLE {
        *in_path = true;
        value.push(QUOTE_SINGLE);
        *index += 1;
        return true;
    }

    // handle bracket open
    if current_char == BRACKET_OPEN {
        *range_depth = 1;
        value.push(BRACKET_OPEN);
        *index += 1;
        return true;
    }

    // handle error start
    if current_char == ERROR_START {
        let qualified_reference_error =
            value.ends_with('!') && formula.chars().skip(*index).take(5).eq("#REF!".chars());
        if !value.is_empty() && !qualified_reference_error {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Unknown);
            tokens1.push(obj);
            value.clear();
        }
        *in_error = true;
        value.push(ERROR_START);
        *index += 1;
        return true;
    }

    // handle braces and semicolons (array constructs)
    if handle_array_chars(current_char, value, tokens1, stack) {
        *index += 1;
        return true;
    }

    // handle whitespace
    if current_char == WHITESPACE {
        handle_whitespace(value, tokens1);
        *index += 1;
        // skip consecutive spaces
        let len = formula.chars().count();
        while *index < len && formula.chars().nth(*index).unwrap() == WHITESPACE {
            *index += 1;
        }
        return true;
    }

    // handle multi-character comparators
    if handle_multi_char_comparator(formula, index, value, tokens1) {
        return true;
    }

    // handle infix operators
    if OPERATORS_INFIX.contains(current_char) {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            tokens1.push(obj);
            value.clear();
        }
        let mut obj = FormulaToken::default();
        obj.set_value(current_char);
        obj.set_token_type(FormulaTokenTypes::OperatorInfix);
        tokens1.push(obj);
        *index += 1;
        return true;
    }

    // handle postfix operators
    if OPERATORS_POSTFIX.contains(current_char) {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            tokens1.push(obj);
            value.clear();
        }
        let mut obj = FormulaToken::default();
        obj.set_value(current_char);
        obj.set_token_type(FormulaTokenTypes::OperatorPostfix);
        tokens1.push(obj);
        *index += 1;
        return true;
    }

    // handle subexpression / function start
    if current_char == PAREN_OPEN {
        if value.is_empty() {
            let obj = token!(
                "",
                FormulaTokenTypes::Subexpression,
                FormulaTokenSubTypes::Start
            );
            tokens1.push(obj.clone());
            stack.push(obj);
        } else {
            let obj = token!(
                value.clone(),
                FormulaTokenTypes::Function,
                FormulaTokenSubTypes::Start
            );
            tokens1.push(obj.clone());
            stack.push(obj);
            value.clear();
        }
        *index += 1;
        return true;
    }

    // handle function/operand unions
    if current_char == COMMA {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            tokens1.push(obj);
            value.clear();
        }

        let Some(obj) = stack.last() else {
            return false;
        };
        // Inspect the frame without rewriting it: ARRAYROW/ARRAY identity is
        // needed to reject a later unmatched delimiter without consuming a
        // surrounding function or subexpression.

        if obj.get_token_type() == &FormulaTokenTypes::Function {
            let mut op = FormulaToken::default();
            op.set_value(",");
            op.set_token_type(FormulaTokenTypes::OperatorInfix);
            op.set_token_sub_type(FormulaTokenSubTypes::Union);
            tokens1.push(op);
        } else {
            let mut arg = FormulaToken::default();
            arg.set_value(",");
            arg.set_token_type(FormulaTokenTypes::Argument);
            tokens1.push(arg);
        }
        *index += 1;
        return true;
    }

    // handle subexpression/function stop
    if current_char == PAREN_CLOSE {
        if stack
            .last()
            .is_none_or(|frame| matches!(frame.get_value(), "ARRAY" | "ARRAYROW"))
        {
            retain_unmatched_delimiter(current_char, value, tokens1);
            *index += 1;
            return true;
        }
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            tokens1.push(obj);
            value.clear();
        }
        let mut obj = stack.pop().unwrap();
        obj.set_value("");
        obj.set_token_sub_type(FormulaTokenSubTypes::Stop);
        tokens1.push(obj);
        *index += 1;
        return true;
    }

    false
}

fn retain_unmatched_delimiter(delimiter: char, value: &mut String, tokens: &mut Vec<FormulaToken>) {
    if !value.is_empty() {
        tokens.push(token!(
            value.clone(),
            FormulaTokenTypes::Operand,
            FormulaTokenSubTypes::Nothing
        ));
        value.clear();
    }
    tokens.push(token!(
        delimiter,
        FormulaTokenTypes::Unknown,
        FormulaTokenSubTypes::Nothing
    ));
}

fn handle_array_chars(
    current_char: char,
    value: &mut String,
    tokens1: &mut Vec<FormulaToken>,
    stack: &mut Vec<FormulaToken>,
) -> bool {
    // handle {
    if current_char == BRACE_OPEN {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Unknown);
            tokens1.push(obj);
            value.clear();
        }
        let arr_start = token!(
            "ARRAY",
            FormulaTokenTypes::Function,
            FormulaTokenSubTypes::Start
        );
        tokens1.push(arr_start.clone());
        stack.push(arr_start);

        let arr_row = token!(
            "ARRAYROW",
            FormulaTokenTypes::Function,
            FormulaTokenSubTypes::Start
        );
        tokens1.push(arr_row.clone());
        stack.push(arr_row);
        return true;
    }

    if matches!(current_char, SEMICOLON | BRACE_CLOSE) {
        let row_frame = stack
            .last()
            .is_some_and(|frame| frame.get_value() == "ARRAYROW");
        let array_frame = stack
            .len()
            .checked_sub(2)
            .and_then(|index| stack.get(index))
            .is_some_and(|frame| frame.get_value() == "ARRAY");
        if !row_frame || !array_frame {
            retain_unmatched_delimiter(current_char, value, tokens1);
            return true;
        }
    }

    // handle ;
    if current_char == SEMICOLON {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            tokens1.push(obj);
            value.clear();
        }
        let mut obj = stack.pop().unwrap();
        obj.set_value("");
        obj.set_token_sub_type(FormulaTokenSubTypes::Stop);
        tokens1.push(obj);

        let mut comma = FormulaToken::default();
        comma.set_value(",");
        comma.set_token_type(FormulaTokenTypes::Argument);
        tokens1.push(comma);

        let arr_row = token!(
            "ARRAYROW",
            FormulaTokenTypes::Function,
            FormulaTokenSubTypes::Start
        );
        tokens1.push(arr_row.clone());
        stack.push(arr_row);
        return true;
    }

    // handle }
    if current_char == BRACE_CLOSE {
        if !value.is_empty() {
            let mut obj = FormulaToken::default();
            obj.set_value(value.clone());
            obj.set_token_type(FormulaTokenTypes::Operand);
            tokens1.push(obj);
            value.clear();
        }
        let mut obj_end = stack.pop().unwrap();
        obj_end.set_value("");
        obj_end.set_token_sub_type(FormulaTokenSubTypes::Stop);
        tokens1.push(obj_end);

        let mut arr_end = stack.pop().unwrap();
        arr_end.set_value("");
        arr_end.set_token_sub_type(FormulaTokenSubTypes::Stop);
        tokens1.push(arr_end);
        return true;
    }

    false
}

fn handle_whitespace(value: &mut String, tokens1: &mut Vec<FormulaToken>) {
    if !value.is_empty() {
        let mut obj = FormulaToken::default();
        obj.set_value(value.clone());
        obj.set_token_type(FormulaTokenTypes::Operand);
        tokens1.push(obj);
        value.clear();
    }
    let mut space = FormulaToken::default();
    space.set_value("");
    space.set_token_type(FormulaTokenTypes::Whitespace);
    tokens1.push(space);
}

fn handle_multi_char_comparator(
    formula: &str,
    index: &mut usize,
    value: &mut String,
    tokens1: &mut Vec<FormulaToken>,
) -> bool {
    let formula_length = formula.chars().count();
    if (*index + 2) <= formula_length {
        let next_two = formula.chars().skip(*index).take(2).collect::<String>();
        if COMPARATORS_MULTI.iter().any(|&x| x == next_two) {
            // flush current value
            if !value.is_empty() {
                let mut obj = FormulaToken::default();
                obj.set_value(value.clone());
                obj.set_token_type(FormulaTokenTypes::Operand);
                tokens1.push(obj);
                value.clear();
            }
            let mut obj = FormulaToken::default();
            obj.set_value(next_two);
            obj.set_token_type(FormulaTokenTypes::OperatorInfix);
            obj.set_token_sub_type(FormulaTokenSubTypes::Logical);
            tokens1.push(obj);
            *index += 2;
            return true;
        }
    }
    false
}

fn cleanup_tokens(tokens1: &[FormulaToken], tokens2: &mut Vec<FormulaToken>) {
    let token_count = tokens1.len();
    let mut value = String::new();
    for i in 0..token_count {
        let token = &tokens1[i];
        if token.get_token_type() != &FormulaTokenTypes::Whitespace {
            tokens2.push(token.clone());
            continue;
        }

        let mut previous_token: Option<&FormulaToken> = None;
        let mut next_token: Option<&FormulaToken> = None;

        if i > 0 {
            previous_token = tokens1.get(i - 1);
        }
        if i + 1 < token_count {
            next_token = tokens1.get(i + 1);
        }

        if let Some(p) = previous_token {
            if !is_operand_or_close(p) {
                continue;
            }
        }
        if let Some(n) = next_token {
            if !is_operand_or_open(n) {
                continue;
            }
        }
        tokens2.push(token!(
            value.clone(),
            FormulaTokenTypes::OperatorInfix,
            FormulaTokenSubTypes::Intersection
        ));
        value.clear();

        if let Some(n) = next_token {
            if !is_operand_or_open(n) {
                continue;
            }
        }
        tokens2.push(token!(
            value.clone(),
            FormulaTokenTypes::OperatorInfix,
            FormulaTokenSubTypes::Intersection
        ));
        value.clear();
    }
}

#[allow(clippy::ref_option_ref)]
fn should_token_be_math(previous_token: Option<FormulaToken>) -> bool {
    if previous_token.is_none() {
        return false;
    }
    let t = previous_token.unwrap();
    (t.get_token_type() == &FormulaTokenTypes::Function
        && t.get_token_sub_type() == &FormulaTokenSubTypes::Stop)
        || (t.get_token_type() == &FormulaTokenTypes::Subexpression
            && t.get_token_sub_type() == &FormulaTokenSubTypes::Stop)
        || (t.get_token_type() == &FormulaTokenTypes::OperatorPostfix)
        || (t.get_token_type() == &FormulaTokenTypes::Operand)
}

fn is_operand_or_close(token: &FormulaToken) -> bool {
    ((token.get_token_type() == &FormulaTokenTypes::Function)
        && (token.get_token_sub_type() == &FormulaTokenSubTypes::Stop))
        || ((token.get_token_type() == &FormulaTokenTypes::Subexpression)
            && (token.get_token_sub_type() == &FormulaTokenSubTypes::Stop))
        || (token.get_token_type() == &FormulaTokenTypes::Operand)
}

fn is_operand_or_open(token: &FormulaToken) -> bool {
    ((token.get_token_type() == &FormulaTokenTypes::Function)
        && (token.get_token_sub_type() == &FormulaTokenSubTypes::Start))
        || ((token.get_token_type() == &FormulaTokenTypes::Subexpression)
            && (token.get_token_sub_type() == &FormulaTokenSubTypes::Start))
        || (token.get_token_type() == &FormulaTokenTypes::Operand)
}

pub(crate) fn render(formula_token_list: &[FormulaToken]) -> String {
    let mut result = String::new();
    for token in formula_token_list {
        if token.get_token_type() == &FormulaTokenTypes::Function
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Start
        {
            result = format!("{}{}", result, token.get_value());
            result = format!("{result}{PAREN_OPEN}");
        } else if token.get_token_type() == &FormulaTokenTypes::Function
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Stop
        {
            result = format!("{result}{PAREN_CLOSE}");
        } else if token.get_token_type() == &FormulaTokenTypes::Subexpression
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Start
        {
            result = format!("{result}{PAREN_OPEN}");
        } else if token.get_token_type() == &FormulaTokenTypes::Subexpression
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Stop
        {
            result = format!("{result}{PAREN_CLOSE}");
        } else if token.get_token_type() == &FormulaTokenTypes::Operand
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Text
        {
            result = format!("{result}{QUOTE_DOUBLE}");
            result = format!("{}{}", result, token.get_value());
            result = format!("{result}{QUOTE_DOUBLE}");
        } else if token.get_token_type() == &FormulaTokenTypes::OperatorInfix
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Intersection
        {
            result = format!("{result}{WHITESPACE}");
        } else {
            result = format!("{}{}", result, token.get_value());
        }
    }
    result
}

pub fn adjustment_formula_coordinate(
    token_list: &mut [FormulaToken],
    offset_col_num: i32,
    offset_row_num: i32,
) {
    for token in token_list.iter_mut() {
        if token.get_token_type() == &FormulaTokenTypes::Operand
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
        {
            let (sheet_name, range) = split_address(token.get_value());
            let sheet_name = unescape_sheet_name(sheet_name);
            let mut coordinate_list_new: Vec<String> = Vec::new();
            let coordinate_list = get_split_range(range);
            let mut has_error = false;
            for coordinate in &coordinate_list {
                let (mut col_num, mut row_num, is_lock_col, is_lock_row) =
                    index_from_coordinate(coordinate);
                if col_num.is_some() || row_num.is_some() {
                    if let Some(col_num) = col_num.as_mut() {
                        if !is_lock_col.unwrap_or_default() {
                            let calc_col_num =
                                num_traits::cast::<_, i32>(*col_num).unwrap() + offset_col_num;
                            if calc_col_num < 1 {
                                has_error = true;
                                break;
                            }
                            *col_num = num_traits::cast::<_, u32>(calc_col_num).unwrap();
                        }
                    }
                    if let Some(row_num) = row_num.as_mut() {
                        if !is_lock_row.unwrap_or_default() {
                            let calc_row_num =
                                num_traits::cast::<_, i32>(*row_num).unwrap() + offset_row_num;
                            if calc_row_num < 1 {
                                has_error = true;
                                break;
                            }
                            *row_num = num_traits::cast::<_, u32>(calc_row_num).unwrap();
                        }
                    }
                    let mut new_coordinate = String::new();
                    if let Some(col_num) = col_num {
                        if is_lock_col.unwrap_or_default() {
                            new_coordinate.push('$');
                        }
                        new_coordinate.push_str(&string_from_column_index(col_num));
                    }
                    if let Some(row_num) = row_num {
                        if is_lock_row.unwrap_or_default() {
                            new_coordinate.push('$');
                        }
                        new_coordinate.push_str(&row_num.to_string());
                    }
                    coordinate_list_new.push(new_coordinate);
                } else {
                    coordinate_list_new.push((*coordinate).to_string());
                }
            }
            if has_error {
                token.set_value("#REF!");
                token.set_token_sub_type(FormulaTokenSubTypes::Error);
            } else {
                let new_value = join_address(&sheet_name, &get_join_range(&coordinate_list_new));
                token.set_value(new_value);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn adjustment_insert_formula_coordinate(
    token_list: &mut [FormulaToken],
    root_col_num: u32,
    offset_col_num: u32,
    root_row_num: u32,
    offset_row_num: u32,
    worksheet_name: &str,
    self_worksheet_name: &str,
    ignore_worksheet: bool,
) -> String {
    for token in token_list.iter_mut() {
        if token.get_token_type() == &FormulaTokenTypes::Operand
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
        {
            let (sheet_name, range) = split_address(token.get_value());
            let sheet_name = unescape_sheet_name(sheet_name);
            if ignore_worksheet
                || (sheet_name.is_empty() && worksheet_name == self_worksheet_name)
                || (sheet_name == worksheet_name)
            {
                let mut coordinate_list_new: Vec<String> = Vec::new();
                let coordinate_list = get_split_range(range);
                for coordinate in &coordinate_list {
                    let cell = index_from_coordinate(coordinate);
                    if let Some(cell_0) = cell.0 {
                        if let Some(cell_1) = cell.1 {
                            let mut col_num = cell_0;
                            let mut row_num = cell_1;
                            let is_lock_col = cell.2.unwrap_or(false);
                            let is_lock_row = cell.3.unwrap_or(false);
                            if !is_lock_col {
                                col_num = adjustment_insert_coordinate(
                                    col_num,
                                    root_col_num,
                                    offset_col_num,
                                );
                            }
                            if !is_lock_row {
                                row_num = adjustment_insert_coordinate(
                                    row_num,
                                    root_row_num,
                                    offset_row_num,
                                );
                            }
                            let new_corrdinate = coordinate_from_index_with_lock(
                                col_num,
                                row_num,
                                is_lock_col,
                                is_lock_row,
                            );
                            coordinate_list_new.push(new_corrdinate);
                        } else {
                            coordinate_list_new.push((*coordinate).to_string());
                        }
                    } else {
                        coordinate_list_new.push((*coordinate).to_string());
                    }
                }
                let new_value = join_address(&sheet_name, &get_join_range(&coordinate_list_new));
                token.set_value(new_value);
            }
        }
    }
    render(token_list)
}

#[allow(clippy::too_many_arguments)]
pub fn adjustment_remove_formula_coordinate(
    token_list: &mut [FormulaToken],
    root_col_num: u32,
    offset_col_num: u32,
    root_row_num: u32,
    offset_row_num: u32,
    worksheet_name: &str,
    self_worksheet_name: &str,
    ignore_worksheet: bool,
) -> String {
    for token in token_list.iter_mut() {
        if token.get_token_type() == &FormulaTokenTypes::Operand
            && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
        {
            let (sheet_name, range) = split_address(token.get_value());
            let sheet_name = unescape_sheet_name(sheet_name);
            if ignore_worksheet
                || (sheet_name.is_empty() && worksheet_name == self_worksheet_name)
                || (sheet_name == worksheet_name)
            {
                let mut coordinate_list_new: Vec<String> = Vec::new();
                let coordinate_list = get_split_range(range);
                for coordinate in &coordinate_list {
                    let cell = index_from_coordinate(coordinate);
                    if let Some(cell_0) = cell.0 {
                        let mut col_num = cell_0;
                        let mut row_num = cell.1.unwrap();
                        let is_lock_col = cell.2.unwrap();
                        let is_lock_row = cell.3.unwrap();
                        if !is_lock_col {
                            col_num =
                                adjustment_remove_coordinate(col_num, root_col_num, offset_col_num);
                        }
                        if !is_lock_row {
                            row_num =
                                adjustment_remove_coordinate(row_num, root_row_num, offset_row_num);
                        }
                        let new_corrdinate = coordinate_from_index_with_lock(
                            col_num,
                            row_num,
                            is_lock_col,
                            is_lock_row,
                        );
                        coordinate_list_new.push(new_corrdinate);
                    } else {
                        coordinate_list_new.push((*coordinate).to_string());
                    }
                }
                let new_value = join_address(&sheet_name, &get_join_range(&coordinate_list_new));
                token.set_value(new_value);
            }
        }
    }
    render(token_list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_reference_projection_is_complete_ordered_and_source_backed() {
        let alphabet = [
            'A', '1', '!', '$', '(', ')', '{', '}', ',', ';', '[', ']', '\'', '"', '#', ' ', '+',
            '\u{00e9}', '\u{6570}',
        ];
        let mut seed = 0x7a5b_31c9_u32;
        for case in 0..2048 {
            let mut input = String::from("=");
            for _ in 0..(case % 48) {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let index = usize::try_from(seed).unwrap() % alphabet.len();
                input.push(alphabet[index]);
            }
            let tokens = parse_chart_formula_tokens(&input);
            let expected: Vec<_> = tokens
                .iter()
                .filter(|token| {
                    token.get_token_type() == &FormulaTokenTypes::Operand
                        && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
                })
                .map(|token| {
                    (
                        token
                            .source_span()
                            .expect("every genuine reference has its raw span"),
                        token.get_value(),
                    )
                })
                .collect();
            let projected = chart_formula_operands(&input);
            let mut end = 0;
            for operand in &projected {
                assert!(operand.span.start >= end, "overlapping spans: {input:?}");
                assert_eq!(operand.text, &input[operand.span.clone()]);
                end = operand.span.end;
            }
            let actual: Vec<_> = projected
                .iter()
                .map(|operand| (operand.span.clone(), operand.text))
                .collect();
            assert_eq!(
                actual, expected,
                "projection must not drop a reference: {input:?}"
            );
        }
    }

    #[test]
    fn unfinished_lexical_owners_are_not_projected_as_editable_references() {
        for input in [
            "=Data!A1+\"Other!B2",
            "=Data!A1+'Other!B2",
            "=Data!A1+[Book Other!B2",
            "=Data!A1+Other!#BOGUS",
        ] {
            let operands = chart_formula_operands(input);
            assert_eq!(operands.len(), 1, "{input}");
            assert_eq!(operands[0].text, "Data!A1");
        }
        for input in [
            "Data!A1",
            "=Data!A1",
            "'\u{6570}\u{636e}'!A1",
            "='\u{6570}\u{636e}'!A1",
        ] {
            let operands = chart_formula_operands(input);
            assert_eq!(operands.len(), 1);
            assert_eq!(operands[0].text, &input[operands[0].span.clone()]);
        }
    }

    #[test]
    fn malformed_array_delimiters_preserve_lexemes_and_valid_sibling_spans() {
        for input in [
            "=Data!A1}+Other!B2",
            "=Data!A1;+Other!B2",
            "=SUM(Data!A1;Other!B2)",
            "={Data!A1)}+Other!B2",
            "=Data!A1)+Other!B2",
        ] {
            let tokens = parse_chart_formula_tokens(input);
            let references: Vec<_> = tokens
                .iter()
                .filter(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Range)
                .map(|token| {
                    assert_eq!(
                        &input[token.source_span().expect("native operand span")],
                        token.get_value()
                    );
                    token.get_value()
                })
                .collect();
            assert_eq!(references, ["Data!A1", "Other!B2"], "{input}: {tokens:?}");
            assert!(
                tokens
                    .iter()
                    .any(|token| token.get_token_type() == &FormulaTokenTypes::Unknown),
                "malformed punctuation must not be discarded"
            );
        }
    }

    #[test]
    fn array_rows_with_commas_keep_balanced_synthetic_frames_and_operand_spans() {
        let input = "={Data!A1,Other!B2;Data!C3,Other!D4}";
        let tokens = parse_chart_formula_tokens(input);
        let references: Vec<_> = tokens
            .iter()
            .filter(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Range)
            .map(|token| {
                assert_eq!(
                    &input[token.source_span().expect("array operand span")],
                    token.get_value()
                );
                token.get_value()
            })
            .collect();
        assert_eq!(references, ["Data!A1", "Other!B2", "Data!C3", "Other!D4"]);
        let starts = tokens
            .iter()
            .filter(|token| {
                token.get_token_type() == &FormulaTokenTypes::Function
                    && token.get_token_sub_type() == &FormulaTokenSubTypes::Start
            })
            .count();
        let stops = tokens
            .iter()
            .filter(|token| {
                token.get_token_type() == &FormulaTokenTypes::Function
                    && token.get_token_sub_type() == &FormulaTokenSubTypes::Stop
            })
            .count();
        assert_eq!(starts, 3);
        assert_eq!(stops, starts);
        assert!(
            !tokens
                .iter()
                .any(|token| token.get_token_type() == &FormulaTokenTypes::Unknown)
        );
    }

    #[test]
    fn qualified_reference_errors_are_one_lossless_reference_operand() {
        for reference in [
            "Data!#REF!",
            "'Bob''s \u{6570}\u{636e}'!#REF!",
            "First:Last!#REF!",
            "'[Book.xlsx]Data'!#REF!",
        ] {
            let input = format!("=SUM({reference},#REF!,\"{reference}\",Data!$A$1)");
            let tokens = parse_chart_formula_tokens(&input);
            let ranges: Vec<_> = tokens
                .iter()
                .filter(|token| {
                    token.get_token_type() == &FormulaTokenTypes::Operand
                        && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
                })
                .collect();
            assert_eq!(ranges.len(), 2, "{reference}: {tokens:?}");
            assert_eq!(ranges[0].get_value(), reference);
            assert_eq!(
                &input[ranges[0].source_span().expect("qualified error span")],
                reference
            );
            assert_eq!(ranges[1].get_value(), "Data!$A$1");
            let errors: Vec<_> = tokens
                .iter()
                .filter(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Error)
                .collect();
            assert_eq!(errors.len(), 1, "bare error remains distinct");
            assert_eq!(errors[0].get_value(), "#REF!");
            assert!(
                tokens
                    .iter()
                    .filter(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Text)
                    .all(|token| token.source_span().is_none())
            );
        }
    }

    #[test]
    fn reference_operand_spans_keep_exact_utf8_quoting_and_ignore_string_literals() {
        let input = "=SUM( 'Bob''s 数据'!$B$2:$B$5 ,\"Data!B2\",'Bob''s 数据'!$B$2:$B$5)";
        let tokens = parse_chart_formula_tokens(input);
        let references: Vec<_> = tokens
            .iter()
            .filter(|token| {
                token.get_token_type() == &FormulaTokenTypes::Operand
                    && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
            })
            .collect();
        assert_eq!(references.len(), 2);
        let spans: Vec<_> = references
            .iter()
            .map(|token| token.source_span().unwrap())
            .collect();
        assert_ne!(spans[0], spans[1]);
        for span in spans {
            assert_eq!(&input[span], "'Bob''s 数据'!$B$2:$B$5");
        }
        let literal = tokens
            .iter()
            .find(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Text)
            .unwrap();
        assert!(literal.source_span().is_none());
        let mut changed = (*references[0]).clone();
        changed.set_value("Data!C3");
        assert!(
            changed.source_span().is_none(),
            "edited token must not retain a stale source span"
        );
    }

    #[test]
    fn operand_spans_preserve_names_external_books_and_whole_axes() {
        let input = "=SUM('[Book.xlsx]Data'!$A$1,MyNamedRange,Sheet1!$C:$D,1:3)";
        let tokens = parse_chart_formula_tokens(input);
        let values: Vec<_> = tokens
            .iter()
            .filter(|token| {
                token.get_token_type() == &FormulaTokenTypes::Operand
                    && token.get_token_sub_type() == &FormulaTokenSubTypes::Range
            })
            .map(|token| {
                let span = token.source_span().expect("raw operand span");
                assert_eq!(&input[span], token.get_value());
                token.get_value().to_owned()
            })
            .collect();
        assert_eq!(
            values,
            vec![
                "'[Book.xlsx]Data'!$A$1",
                "MyNamedRange",
                "Sheet1!$C:$D",
                "1:3"
            ]
        );
    }

    #[test]
    fn chart_formula_adapter_spans_always_index_the_original_text() {
        for input in ["'Bob''s 数据'!$B$2:$B$5", "='Bob''s 数据'!$B$2:$B$5"] {
            let tokens = parse_chart_formula_tokens(input);
            let token = tokens
                .iter()
                .find(|token| token.get_token_sub_type() == &FormulaTokenSubTypes::Range)
                .unwrap();
            assert_eq!(
                &input[token.source_span().unwrap()],
                "'Bob''s 数据'!$B$2:$B$5"
            );
        }
    }

    fn adjusted_formula(formula: &str, offset_col_num: i32, offset_row_num: i32) -> String {
        let mut tokens = parse_to_tokens(formula);
        adjustment_formula_coordinate(&mut tokens, offset_col_num, offset_row_num);
        render(&tokens)
    }

    #[test]
    fn test() {
        let formula = "=10+9";
        assert_eq!(
            format!("={}", render(parse_to_tokens(formula).as_ref())),
            formula
        );

        let formula = "=SUM(E7:I7)";
        assert_eq!(
            format!("={}", render(parse_to_tokens(formula).as_ref())),
            formula
        );

        let formula = "=SUM(Sheet2!E7:I7)";
        assert_eq!(
            format!("={}", render(parse_to_tokens(formula).as_ref())),
            formula
        );

        let formula = "=\"TEST\"";
        assert_eq!(
            format!("={}", render(parse_to_tokens(formula).as_ref())),
            formula
        );
    }

    #[test]
    fn shared_formula_adjustment_handles_whole_column_references() {
        assert_eq!(
            adjusted_formula("=IF(G3=\"\",\"\",VLOOKUP(G3,A:B,2,FALSE))", 0, 1),
            "IF(G4=\"\",\"\",VLOOKUP(G4,A:B,2,FALSE))"
        );
        assert_eq!(
            adjusted_formula("=VLOOKUP(G3,A:B,2,FALSE)", 1, 0),
            "VLOOKUP(H3,B:C,2,FALSE)"
        );
        assert_eq!(
            adjusted_formula("=VLOOKUP(G3,$A:$B,2,FALSE)", 1, 0),
            "VLOOKUP(H3,$A:$B,2,FALSE)"
        );
    }

    #[test]
    fn shared_formula_adjustment_handles_whole_row_references() {
        assert_eq!(adjusted_formula("=SUM(1:3)", 0, 2), "SUM(3:5)");
        assert_eq!(adjusted_formula("=SUM($1:$3)", 0, 2), "SUM($1:$3)");
    }

    #[test]
    fn shared_formula_adjustment_preserves_named_ranges() {
        assert_eq!(
            adjusted_formula("=SUM(MyNamedRange)", 1, 1),
            "SUM(MyNamedRange)"
        );
    }

    #[test]
    fn shared_formula_adjustment_reports_out_of_bounds_whole_references() {
        assert_eq!(adjusted_formula("=SUM(A:B)", -1, 0), "SUM(#REF!)");
        assert_eq!(adjusted_formula("=SUM(1:3)", 0, -1), "SUM(#REF!)");
    }
}

#[cfg(test)]
mod shared_formula_quoted_sheet_tests {
    use super::*;

    #[test]
    fn quoted_sheet_ref_round_trips() {
        let f = r#"IF(A2=1,'(1)'!$E$38&"-x","")"#;
        assert_eq!(render(&parse_to_tokens(format!("={f}"))), f);
    }

    #[test]
    fn shared_adjustment_preserves_quoted_sheet() {
        let mut tokens = parse_to_tokens(r#"=IF(A2=1,'(1)'!$E$38&"-x","")"#);
        let out = adjustment_insert_formula_coordinate(&mut tokens, 0, 0, 2, 1, "", "Main", true);
        assert!(
            out.contains("'(1)'!$E$38"),
            "quoted sheet ref corrupted: {out}"
        );
        assert!(
            !out.contains("(1)!$E$38"),
            "sheet name lost its quotes: {out}"
        );
    }
}
