use nom::{
    AsChar, Err, IResult, Input, Parser,
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    character::complete::{char, multispace0},
    combinator::map_res,
    error::{FromExternalError, ParseError},
    multi::many0,
    number::complete::float,
    sequence::{delimited, preceded, separated_pair},
};
use nom_language::error::{VerboseError, convert_error};
use std::{collections::BTreeMap, num::ParseIntError};

use sailor_platform::platform::{self, FileWatcher, Watcher};

/// The default stylesheet, embedded so it is available on the web.
const DEFAULT_STYLE: &str = include_str!("../../../config/style.css");

/// Tries to parse an entire stylesheet.
pub fn try_parse_styles(style: &str) -> Option<Vec<Rule>> {
    let style: &str = &strip_comments(style);
    match rules::<VerboseError<&str>>(style) {
        Ok((remaining, s)) => {
            // many0 stops at the first byte it can't parse and silently keeps the rest.
            // Surface that instead of rendering nothing.
            if !remaining.trim().is_empty() {
                let at = style.len() - remaining.len();
                let line = style[..at].bytes().filter(|&b| b == b'\n').count() + 1;
                let snippet: String = remaining.trim_start().chars().take(60).collect();
                log::warn!(
                    "stylesheet: stopped parsing at line {line}, {} rule(s) loaded. \
                     unparsed: {snippet:?}",
                    s.len()
                );
            }
            Some(s)
        }
        Err(Err::Error(e)) | Err(Err::Failure(e)) => {
            log::info!("Failed to load stylesheet.");
            log::info!("Trace: {}", convert_error(style, e));
            None
        }
        Err(Err::Incomplete(_)) => {
            log::info!("Unexpected EOF loading the stylesheet.");
            None
        }
    }
}

pub struct RulesCache {
    buffer: String,
    file_path: String,
    pub rules: Vec<Rule>,
    watcher: FileWatcher,
}

impl RulesCache {
    /// Tries to create a new CSS rule cache from a given CSS file path.
    ///
    /// Natively the file is read from disk and watched for hot-reloading; on the
    /// web the embedded default stylesheet is used and there is nothing to watch.
    pub fn try_load_from_file(file_path: impl Into<String>) -> Option<Self> {
        let file_path = file_path.into();
        let buffer = platform::read_to_string(&file_path, DEFAULT_STYLE);
        let rules = try_parse_styles(&buffer)?;
        let watcher = FileWatcher::watch(&[&file_path]);

        Some(Self {
            buffer,
            file_path,
            rules,
            watcher,
        })
    }

    /// Returns all Rules that match a given selector.
    ///
    /// E.g. `layer` does not match the `layer[zoom=5]` rule selector.
    /// On the contrary, `layer[zoom=5]` matches the `layer` rule selector.
    pub fn get_matching_rules(&self, selector: &Selector) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|rule| selector.matches(&rule.selector))
            .collect()
    }

    pub fn get_matching_rules_mut(&mut self, selector: &Selector) -> Vec<&mut Rule> {
        self.rules
            .iter_mut()
            .filter(|rule| selector.matches(&rule.selector))
            .collect()
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn try_get_rule_mut(&mut self, selector: Selector) -> Option<&mut Rule> {
        self.rules.iter_mut().find(|rule| selector == rule.selector)
    }

    /// Reloads the stylesheet if the watcher reported a change.
    ///
    /// Returns whether a successful update happened (always `false` on the web,
    /// where nothing is watched).
    pub fn update(&mut self) -> bool {
        if !self.watcher.changed() {
            return false;
        }
        let buffer = platform::read_to_string(&self.file_path, DEFAULT_STYLE);
        match try_parse_styles(&buffer) {
            Some(rules) => {
                self.buffer = buffer;
                self.rules = rules;
                true
            }
            None => false,
        }
    }

    /// Saves the edited stylesheet back to disk (a no-op on the web).
    pub fn try_save_to_file(&mut self) {
        platform::write_bytes(&self.file_path, self.buffer.as_bytes());
    }

    pub fn buffer_mut(&mut self) -> &mut String {
        &mut self.buffer
    }
}

/// A single CSS rule including it's selector.
#[derive(Debug)]
pub struct Rule {
    /// The selector that the rule is intended for.
    pub selector: Selector,
    /// The key/value pairs the rule holds.
    pub kvs: BTreeMap<String, CSSValue>,
}

/// A single CSS selector.
#[derive(Debug, Clone, PartialEq, Eq, Hash, deepsize::DeepSizeOf, Default)]
pub struct Selector {
    /// The type a selector matches.
    /// E.g. `"layer"`.
    pub typ: Option<String>,
    /// The id a selector matches.
    /// E.g. `"0"`.
    pub id: Option<String>,
    /// The classes a selector matches.
    /// E.g. `["landmark", "forest"]`.
    pub classes: Vec<String>,
    /// The name a selector matches.
    /// E.g. `"water"`.
    pub any: BTreeMap<String, String>,
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut selector = self.typ.clone().unwrap_or_default();

        if let Some(id) = self.id.as_ref() {
            selector += id
        }

        for class in &self.classes {
            selector += ".";
            selector += class;
        }
        for (k, v) in &self.any {
            selector += "[";
            selector += k;
            selector += "=";
            selector += v;
            selector += "]";
        }
        write!(f, "({selector})")
    }
}

impl Selector {
    /// Creates a new empty selector.
    pub fn new() -> Self {
        Self {
            typ: None,
            id: None,
            // Usually we don't have many classes so don't allocate much.
            classes: Vec::with_capacity(4),
            any: BTreeMap::new(),
        }
    }

    /// Makes the selector require the type `typ`.
    pub fn with_type(mut self, typ: impl Into<String>) -> Self {
        self.typ = Some(typ.into());
        self
    }

    /// Makes the selector require the id `id`.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Makes the selector require the class `class`.
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// Makes the selector require the kv `key`/`value`.
    pub fn with_any(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.any.insert(key.into(), value.into());
        self
    }

    /// Checks if a subset of criteria of this selector matches all the criteria of another.
    ///
    /// Use example: layer.selector.matches(&landmark_selector)`.
    pub fn matches(&self, other: &Selector) -> bool {
        if let Some(t1) = &other.typ {
            if let Some(t2) = &self.typ {
                if t1 != t2 {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(i1) = &other.id {
            if let Some(i2) = &self.id {
                if i1 != i2 {
                    return false;
                }
            } else {
                return false;
            }
        }

        for (k, v) in &other.any {
            if let Some(value) = self.any.get(k) {
                if value != v {
                    return false;
                }
            } else {
                return false;
            }
        }

        for c in &other.classes {
            if !self.classes.contains(c) {
                return false;
            }
        }

        true
    }

    pub fn size(&self) -> usize {
        use deepsize::DeepSizeOf;
        self.deep_size_of()
    }
}

/// A single part of a selector.
/// Used for parsing only.
#[derive(Debug)]
enum SelectorPart {
    Class(String),
    Id(String),
    Any(String, String),
}

/// Replace `/* ... */` comments with spaces, keeping newlines so line numbers
/// in parse warnings still point at the right place. CSS comments don't nest.
fn strip_comments(style: &str) -> String {
    let mut out = String::with_capacity(style.len());
    let mut rest = style;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("*/").map(|e| e + 2).unwrap_or(after.len());
        for c in after[..end].chars() {
            out.push(if c == '\n' { '\n' } else { ' ' });
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Parses an entire set of rules.
fn rules<'a, E>(input: &'a str) -> IResult<&'a str, Vec<Rule>, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    many0(rule).parse(input)
}

/// Munch all whitespace before and after `f`.
fn whitespace<I, O, E, F>(f: F) -> impl Parser<I, Output = O, Error = E>
where
    I: Clone + PartialEq + Input,
    <I as Input>::Item: AsChar + Clone,
    F: FnMut(I) -> IResult<I, O, E>,
    E: ParseError<I>,
{
    delimited(multispace0, f, multispace0)
}

/// Parse a single rule.
/// E.g. `layer[name=water]{ background-color: #FF0000; }`.
fn rule<'a, E>(input: &'a str) -> IResult<&'a str, Rule, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let (remaining, (selector, _, kvs, _)) = (
        whitespace(selector),
        whitespace(char('{')),
        body,
        whitespace(char('}')),
    )
        .parse(input)?;

    Ok((remaining, Rule { selector, kvs }))
}

/// Parse a single selector.
/// E.g. `layer[name=water].class#id`.
fn selector<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Selector, E> {
    let mut selector: Selector = Default::default();

    // Try parsing the type (Html tag) of a selector.
    let (remaining, typ) = take_while(|c| (c as u8).is_alphanum())(input)?;

    // The type is optional. So if no type was found, set the type to `None`.
    selector.typ = if !typ.is_empty() {
        Some(typ.into())
    } else {
        None
    };

    // Parse all the remaining selector parts.
    let (remaining, pairs) = many0(alt((class, id, any))).parse(remaining)?;

    for pair in pairs {
        match pair {
            SelectorPart::Class(v) => selector.classes.push(v),
            SelectorPart::Id(v) => selector.id = Some(v),
            SelectorPart::Any(k, v) => {
                selector.any.insert(k, v);
            }
        }
    }

    Ok((remaining, selector))
}

/// Parse a single class name.
/// E.g. `.class`.
fn class<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, SelectorPart, E> {
    preceded(char('.'), take_while(|c| (c as u8).is_alphanum()))
        .parse(input)
        .map(|(r, v)| (r, SelectorPart::Class(v.into())))
}

/// Parse a single id name.
/// E.g. `#id`.
fn id<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, SelectorPart, E> {
    preceded(char('#'), take_while(|c| (c as u8).is_alphanum()))
        .parse(input)
        .map(|(r, v)| (r, SelectorPart::Id(v.into())))
}

/// Parse any CSS selector k/v pair.
/// E.g. `[name=water]`
fn any<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, SelectorPart, E> {
    let (remaining, _) = char('[')(input)?;
    let (remaining, name) = take_while(|c| (c as u8).is_alphanum())(remaining)?;
    let (remaining, _) = char('=')(remaining)?;
    let (remaining, _) = char('"')(remaining)?;
    let (remaining, value) = take_while(|c| (c as u8).is_alphanum())(remaining)?;
    let (remaining, _) = char('"')(remaining)?;
    let (remaining, _) = char(']')(remaining)?;
    Ok((remaining, SelectorPart::Any(name.into(), value.into())))
}

/// Parses the body of a CSS rule.
/// E.g. `{}`.
fn body<'a, E>(input: &'a str) -> IResult<&'a str, std::collections::BTreeMap<String, CSSValue>, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let mut hm = std::collections::BTreeMap::new();
    many0(kv).parse(input).map(|v| {
        v.1.into_iter().for_each(|v| {
            hm.insert(v.0.into(), v.1);
        });
        (v.0, hm)
    })
}

/// Parses a single CSS k/v pair.
/// E.g. `background-color: #FF0000;`.
fn kv<'a, E>(input: &'a str) -> IResult<&'a str, (&'a str, CSSValue), E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let (remaining, (kv, _)) =
        (separated_pair(css_name, char(':'), css_value), char(';')).parse(input)?;
    Ok((remaining, kv))
}

/// Parses a CSS qualified name.
/// Can contain alphanumeric characters and '-'.
fn css_name<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    whitespace(take_while(|c| (c as u8).is_alphanum() || c == '-')).parse(input)
}

/// Parses a single CSS qualified value.
fn css_value<'a, E>(input: &'a str) -> IResult<&'a str, CSSValue, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    alt((
        whitespace(hex_color),
        whitespace(rgba_color),
        whitespace(rgb_color),
        whitespace(px_value),
        whitespace(world_value),
        whitespace(unitless_value),
        whitespace(string),
    ))
    .parse(input)
}

#[derive(Debug, Copy, Clone)]
pub enum Number {
    Px(f32),
    Unitless(f32),
    World(f32),
}

/// Any type of CSS value.
#[derive(Debug, Clone)]
pub enum CSSValue {
    /// Represents any value as a string.
    String(String),
    /// Represents a color.
    Color(Color),
    Number(Number),
}

/// Parses a single CSS qualified string.
/// Can contain alphanumeric characters, '-' and spaces.
fn string<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, value) = whitespace(take_while(|c| {
        (c as u8).is_alphanum() || c == '-' || c == ' '
    }))
    .parse(input)?;

    Ok((input, CSSValue::String(value.into())))
}

/// Parses a single CSS px value.
fn px_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, (value, _)) = (float, tag("px")).parse(input)?;

    Ok((input, CSSValue::Number(Number::Px(value))))
}

fn world_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, (value, _)) = (float, tag("w")).parse(input)?;
    Ok((input, CSSValue::Number(Number::World(value))))
}

/// Parses a single CSS unitless value.
fn unitless_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, value) = float(input)?;

    Ok((input, CSSValue::Number(Number::Unitless(value))))
}

/// A struct to represent any RGB color.
#[derive(Debug, PartialEq, Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
}

/// Converts a hex string into an `u8`.
fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(input, 16)
}

/// Parse an actual hex code.
fn hex_primary<'a, E>(input: &'a str) -> IResult<&'a str, u8, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    map_res(take_while_m_n(2, 2, char::is_hex_digit), from_hex).parse(input)
}

/// Parse a single hex color code including the `#`.
fn hex_color<'a, E>(input: &'a str) -> IResult<&'a str, CSSValue, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let (input, _) = tag("#")(input)?;
    let (input, (r, g, b)) = (hex_primary, hex_primary, hex_primary).parse(input)?;

    Ok((
        input,
        CSSValue::Color(Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }),
    ))
}

fn u8<'a, E>(input: &'a str) -> IResult<&'a str, u8, E>
where
    E: ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    use std::str::FromStr;
    map_res(take_while(|c: char| c.is_ascii_digit()), u8::from_str).parse(input)
}

/// Parse a single hex color code including the `#`.
fn rgba_color<'a, E>(input: &'a str) -> IResult<&'a str, CSSValue, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let (input, _) = whitespace(tag("rgba(")).parse(input)?;
    let (input, (r, _, g, _, b, _, a)) = (
        u8,
        whitespace(char(',')),
        u8,
        whitespace(char(',')),
        u8,
        whitespace(char(',')),
        float,
    )
        .parse(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((
        input,
        CSSValue::Color(Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a,
        }),
    ))
}

/// Parse a single hex color code including the `#`.
fn rgb_color<'a, E>(input: &'a str) -> IResult<&'a str, CSSValue, E>
where
    E: ParseError<&'a str> + ParseError<&'a str> + FromExternalError<&'a str, ParseIntError>,
{
    let (input, _) = whitespace(tag("rgb(")).parse(input)?;
    let (input, (r, _, g, _, b)) =
        (u8, whitespace(char(',')), u8, whitespace(char(',')), u8).parse(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((
        input,
        CSSValue::Color(Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }),
    ))
}

#[test]
fn selector_size() {
    let selector = Selector::default();
    assert_eq!(selector.size(), 96);
}

#[test]
fn comments_are_stripped_and_parse() {
    // leading + inline comment; newlines inside comment preserve line numbers.
    let css = "/* header\nspanning */\nbackground { background-color: red; } /* trailing */";
    assert_eq!(strip_comments(css).lines().count(), css.lines().count());
    assert_eq!(try_parse_styles(css).unwrap().len(), 1);
}
