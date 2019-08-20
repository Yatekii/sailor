extern crate nom;

use nom::number::complete::float;
use nom::error::convert_error;
use nom::error::VerboseError;
use nom::InputTakeAtPosition;
use nom::character::complete::multispace0;
use nom::AsChar;
use nom::error::ParseError;
use nom::sequence::delimited;
use nom::sequence::tuple;
use nom::sequence::separated_pair;
use nom::sequence::preceded;
use nom::multi::many0;
use nom::branch::alt;
use nom::character::is_alphanumeric;
use nom::bytes::complete::{
    take_while,
    take_while_m_n,
    tag,
};
use nom::character::complete::char;
use nom::combinator::map_res;
use nom::{
    IResult,
    Err,
};

use crossbeam_channel::{
    unbounded,
    TryRecvError,
};
use notify::{
    RecursiveMode,
    RecommendedWatcher,
    Watcher,
    EventKind,
    event::{
        ModifyKind,
    },
};
use std::collections::BTreeMap;

/// Tries to parse an entire stylesheet.
pub fn try_parse_styles(style: &str) -> Option<Vec<Rule>> {
    match rules::<VerboseError<&str>>(style) {
        Ok((_, s)) => Some(s),
        Err(Err::Error(e)) | Err(Err::Failure(e)) => {
            log::info!("Failed to load stylesheet.");
            log::info!("Trace: {}", convert_error(style, e));
            None
        },
        Err(Err::Incomplete(_)) => {
            log::info!("Unexpected EOF loading the stylesheet.");
            None
        }
    }
}

pub struct RulesCache {
    pub rules: Vec<Rule>,
    rx: crossbeam_channel::Receiver<std::result::Result<notify::event::Event, notify::Error>>,
    _watcher: RecommendedWatcher,
}

impl RulesCache {
    /// Tries to create a new CSS rule cache from a given CSS file path.
    pub fn try_load_from_file(filename: impl Into<String>) -> Option<Self> {
        let filename = filename.into();

        let contents = std::fs::read_to_string(std::path::Path::new(&filename.clone()))
            .expect("Something went wrong reading the file");

        let (tx, rx) = unbounded();
        
        let mut watcher: RecommendedWatcher = match Watcher::new_immediate(tx) {
            Ok(watcher) => watcher,
            Err(err) => {
                log::info!("Failed to create a watcher for the stylesheet:");
                log::info!("{}", err);
                return None;
            },
        };

        match watcher.watch(&filename, RecursiveMode::Recursive) {
            Ok(_) => {},
            Err(err) => {
                log::info!("Failed to start watching {}:", filename);
                log::info!("{}", err);
                return None;
            },
        };

        let rules = try_parse_styles(&contents)?;

        Some(Self {
            rules: rules,
            rx,
            _watcher: watcher,
        })
    }

    /// Returns all Rules that match a given selector.
    /// 
    /// E.g. `layer` does not match the `layer[zoom=5]` rule selector.
    /// On the contrary, `layer[zoom=5]` matches the `layer` rule selector.
    pub fn get_matching_rules(&self, selector: &Selector) -> Vec<&Rule> {

        self.rules.iter().filter(|rule| selector.matches(&rule.selector)).collect()
    }

    pub fn get_matching_rules_mut(&mut self, selector: &Selector) -> Vec<&mut Rule> {

        self.rules.iter_mut().filter(|rule| selector.matches(&rule.selector)).collect()
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn try_get_rule_mut(&mut self, selector: Selector) -> Option<&mut Rule> {
        self.rules.iter_mut().find(|rule| selector == rule.selector)
    }

    /// Updates the CSS cache from the watched file if there was any changes.
    /// 
    /// Returns whether a successful update happened.
    /// Returns false if there was no changes or if the update failed.
    pub fn update(&mut self) -> bool {
        match self.rx.try_recv() {
            Ok(Ok(notify::event::Event {
                kind: EventKind::Modify(ModifyKind::Data(_)),
                paths,
                ..
            })) => {
                self.try_reload_from_file(&paths[0].as_path())
            },
            // Everything is alright but file wasn't actually changed.
            Ok(Ok(_)) => { false },
            Ok(Err(err)) => {
                log::info!("Something went wrong with the CSS file watcher:\r\n{:?}", err);
                false
            },
            // This happens all the time when there is no new message.
            Err(TryRecvError::Empty) => false,
            Err(err) => {
                log::info!("Something went wrong with the CSS file watcher:\r\n{:?}", err);
                false
            },
        }
    }

    /// Tries reloading the cached styles from a file.
    /// 
    /// Returns `true` if it succeeded.
    /// Returns `false` in any error case.
    fn try_reload_from_file(&mut self, filename: &std::path::Path) -> bool {
        match std::fs::read_to_string(filename) {
            Ok(contents) => {
                self.rules = match try_parse_styles(&contents) {
                    Some(rules) => rules,
                    None => return false,
                }
            },
            Err(err) => {
                log::info!("Failed to read file at {:?}:", filename);
                log::info!("{}", err);
                return false;
            },
        }
        true
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

impl Default for Selector {
    fn default() -> Self {
        Self {
            typ: None,
            id: None,
            classes: vec![],
            any: BTreeMap::new(),
        }
    }
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut selector = self.typ.clone().unwrap_or(String::new());
        self.id.as_ref().map(|id| selector += &id);
        for class in &self.classes {
            selector += ".";
            selector += &class;
        }
        for (k, v) in &self.any {
            selector += "[";
            selector += &k;
            selector += "=";
            selector += &v;
            selector += "]";
        }
        write!(f, "({})", selector)
    }
}

impl Selector {
    /// Creates a new empty selector.
    pub fn new() -> Self {
        Self {
            typ: None,
            id: None,
            classes: vec![],
            any: BTreeMap::new(),
        }
    }

    /// Makes the selector require the type `typ`.
    pub fn with_type(mut self, typ: String) -> Self {
        self.typ = Some(typ);
        self
    }

    /// Makes the selector require the id `id`.
    pub fn _with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    /// Makes the selector require the class `class`.
    pub fn _with_class(mut self, class: String) -> Self {
        self.classes.push(class);
        self
    }

    /// Makes the selector require the kv `key`/`value`.
    pub fn with_any(mut self, key: String, value: String) -> Self {
        self.any.insert(key, value);
        self
    }

    /// Checks if a subset of criteria of this selector matches all the criteria of another.
    /// 
    /// Use example: layer.selector.matches(&landmark_selector)`.
    pub fn matches(&self, other: &Selector) -> bool {
        if let Some(t1) = &other.typ {
            if let Some(t2) = &self.typ {
                if t1 != t2 { return false; }
            } else {
                return false;
            }
        }

        if let Some(i1) = &other.id {
            if let Some(i2) = &self.id {
                if i1 != i2 { return false; }
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
}

/// A single part of a selector.
/// Used for parsing only.
#[derive(Debug)]
enum SelectorPart {
    Class(String),
    Id(String),
    Any(String, String),
}

/// Parses an entire set of rules.
fn rules<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Vec<Rule>, E> {
    many0(rule)(input)
}

/// Munch all whitespace before and after `f`.
fn whitespace<I, O, E, F>(f: F) -> impl Fn(I) -> IResult<I, O, E>
where
    I: Clone + PartialEq + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar + Clone,
    F: Fn(I) -> IResult<I, O, E>,
    E: ParseError<I>,
{
    delimited(multispace0, f, multispace0)
}

/// Parse a single rule.
/// E.g. `layer[name=water]{ background-color: #FF0000; }`.
fn rule<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Rule, E> {
    let (remaining, (selector, _, kvs, _)) = tuple((
        whitespace(selector),
        whitespace(char('{')),
        body,
        whitespace(char('}'))
    ))(input)?;

    Ok((remaining, Rule { selector, kvs }))
}

/// Parse a single selector.
/// E.g. `layer[name=water].class#id`.
fn selector<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Selector, E> {
    let mut selector: Selector = Default::default();
    
    // Try parsing the type (Html tag) of a selector.
    let (remaining, typ) = take_while(|c| is_alphanumeric(c as u8))(input)?;

    // The type is optional. So if no type was found, set the type to `None`.
    selector.typ = if typ.len() > 0 { Some(typ.into()) } else { None };

    // Parse all the remaining selector parts.
    let (remaining, pairs) = many0(alt((class, id, any)))(remaining)?;

    for pair in pairs {
        match pair {
            SelectorPart::Class(v) => selector.classes.push(v),
            SelectorPart::Id(v) => selector.id = Some(v),
            SelectorPart::Any(k, v) => { selector.any.insert(k, v); },
        }
    }

    Ok((remaining, selector))
}

/// Parse a single class name.
/// E.g. `.class`.
fn class<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, SelectorPart, E> {
    preceded(char('.'), take_while(|c| is_alphanumeric(c as u8)))(input).map(|(r, v)| (r, SelectorPart::Class(v.into())))
}

/// Parse a single id name.
/// E.g. `#id`.
fn id<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, SelectorPart, E> {
    preceded(char('#'), take_while(|c| is_alphanumeric(c as u8)))(input).map(|(r, v)| (r, SelectorPart::Id(v.into())))
}

/// Parse any CSS selector k/v pair.
/// E.g. `[name=water]`
fn any<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, SelectorPart, E> {
    let (remaining, _) = char('[')(input)?;
    let (remaining, name) = take_while(|c| is_alphanumeric(c as u8))(remaining)?;
    let (remaining, _) = char('=')(remaining)?;
    let (remaining, value) = take_while(|c| is_alphanumeric(c as u8))(remaining)?;
    let (remaining, _) = char(']')(remaining)?;
    Ok((remaining, SelectorPart::Any(name.into(), value.into())))
}

/// Parses the body of a CSS rule.
/// E.g. `{}`.
fn body<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, std::collections::BTreeMap::<String, CSSValue>, E> {
    let mut hm = std::collections::BTreeMap::new();
    many0(kv)(input).map(|v| { v.1.into_iter().for_each(|v| { hm.insert(v.0.into(), v.1); }); (v.0, hm) })
}

/// Parses a single CSS k/v pair.
/// E.g. `background-color: #FF0000;`.
fn kv<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, (&'a str, CSSValue), E> {
    let (remaining, (kv, _)) = tuple((
        separated_pair(css_name, char(':'), css_value),
        char(';')
    ))(input)?;
    Ok((remaining, kv))
}

/// Parses a CSS qualified name.
/// Can contain alphanumeric characters and '-'.
fn css_name<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    whitespace(take_while(|c| is_alphanumeric(c as u8) || c == '-'))(input)
}

/// Parses a single CSS qualified value.
fn css_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    alt((
        whitespace(hex_color),
        whitespace(rgba_color),
        whitespace(rgb_color),
        whitespace(px_value),
        whitespace(world_value),
        whitespace(unitless_value),
        whitespace(string),
    ))(input)
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
  let (input, value) = whitespace(take_while(|c| is_alphanumeric(c as u8) || c == '-' || c == ' '))(input)?;

  Ok((input, CSSValue::String(value.into())))
}

/// Parses a single CSS px value.
fn px_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
  let (input, (value, _)) = tuple((float, tag("px")))(input)?;

  Ok((input, CSSValue::Number(Number::Px(value))))
}

fn world_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
  let (input, (value, _)) = tuple((float, tag("w")))(input)?;
    println!("{:?}", value);
  Ok((input, CSSValue::Number(Number::World(value))))
}

/// Parses a single CSS unitless value.
fn unitless_value<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
  let (input, value) = float(input)?;

  Ok((input, CSSValue::Number(Number::Unitless(value))))
}

/// A struct to represent any RGB color.
#[derive(Debug,PartialEq, Clone)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0.0, };
    pub const _WHITE: Color = Color { r: 255, g: 255, b: 255, a: 1.0, };
    pub const _BLACK: Color = Color { r:   0, g:   0, b:   0, a: 1.0, };
    pub const RED:   Color = Color { r: 255, g:   0, b:   0, a: 1.0, };
    pub const GREEN: Color = Color { r:   0, g: 255, b:   0, a: 1.0, };
    pub const BLUE:  Color = Color { r:   0, g:   0, b: 255, a: 1.0, };
}

/// Converts a hex string into an `u8`.
fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(input, 16)
}

/// `true` if `c` is a hexadecimal valid digit.
fn is_hex_digit(c: char) -> bool {
    c.is_digit(16)
}

/// Parse an actual hex code.
fn hex_primary<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, u8, E> {
    map_res(
        take_while_m_n(2, 2, is_hex_digit),
        from_hex
    )(input)
}

/// Parse a single hex color code including the `#`.
fn hex_color<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, _) = tag("#")(input)?;
    let (input, (r, g, b)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;

    Ok((input, CSSValue::Color(Color { r, g, b, a: 1.0 })))
}

fn u8<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, u8, E> {
    use std::str::FromStr;
    map_res(
        take_while(|c: char| c.is_digit(10)),
        u8::from_str
    )(input)
}

/// Parse a single hex color code including the `#`.
fn rgba_color<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, _) = whitespace(tag("rgba("))(input)?;
    let (input, (r, _, g, _, b, _, a)) = tuple((
        u8,
        whitespace(char(',')),
        u8,
        whitespace(char(',')),
        u8,
        whitespace(char(',')),
        float,
    ))(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, CSSValue::Color(Color { r, g, b, a })))
}

/// Parse a single hex color code including the `#`.
fn rgb_color<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, CSSValue, E> {
    let (input, _) = whitespace(tag("rgb("))(input)?;
    let (input, (r, _, g, _, b)) = tuple((
        u8,
        whitespace(char(',')),
        u8,
        whitespace(char(',')),
        u8,
    ))(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, CSSValue::Color(Color { r, g, b, a: 1.0 })))
}