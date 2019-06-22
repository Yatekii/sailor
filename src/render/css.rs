extern crate nom;

use nom::InputTakeAtPosition;
use nom::character::complete::multispace0;
use nom::AsChar;
use nom::lib::std::ops::RangeFrom;
use nom::Slice;
use nom::error::ParseError;
use nom::sequence::delimited;
use nom::character::complete::one_of;
use nom::combinator::opt;
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
use nom::IResult;

pub fn parse_styles(style: &str) -> Vec<Rule> {
    dbg!(style);
    let (_, result) = rules(style)
        .expect("Failed to parse stylesheet.");
    result
}

#[derive(Debug)]
pub struct RulesCache {
    pub rules: Vec<Rule>,
}

impl RulesCache {
    pub fn load_from_file(filename: impl Into<String>) -> Self {
        let contents = std::fs::read_to_string(std::path::Path::new(&filename.into()))
            .expect("Something went wrong reading the file");
        Self {
            rules: parse_styles(&contents)
        }
    }

    pub fn get_matching_rules(&self, selector: &Selector) -> Vec<&Rule> {
        self.rules.iter().filter(|rule| selector.matches(&rule.selector)).collect()
    }
}

#[derive(Debug)]
pub struct Rule {
    pub selector: Selector,
    pub kvs: std::collections::HashMap<String, CSSValue>,
}

#[derive(Debug)]
pub struct Selector {
    pub typ: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub name: Option<String>,
}

impl Default for Selector {
    fn default() -> Self {
        Self {
            typ: None,
            id: None,
            classes: vec![],
            name: None,
        }
    }
}

impl Selector {
    pub fn matches(&self, other: &Selector) -> bool {
        dbg!(self);
        dbg!(other);
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

        if let Some(n1) = &other.name {
            if let Some(n2) = &self.name {
                if n1 != n2 { return false; }
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

#[derive(Debug)]
pub enum SelectorPart {
    Class(String),
    Id(String),
    Any(String, String),
}

fn rules(input: &str) -> IResult<&str, Vec<Rule>> {
    many0(rule)(input)
}

fn whitespace<I, O, E, F>(f: F) -> impl Fn(I) -> IResult<I, O, E>
where
    I: Clone + PartialEq + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar + Clone,
    F: Fn(I) -> IResult<I, O, E>,
    E: ParseError<I>,
{
    delimited(multispace0, f, multispace0)
}

fn rule(input: &str) -> IResult<&str, Rule> {
    let (remaining, (selector, _, kvs, _)) = tuple((
        whitespace(selector),
        whitespace(char('{')),
        body,
        whitespace(char('}'))
    ))(input)?;

    Ok(("", Rule { selector, kvs }))
}

fn selector(input: &str) -> IResult<&str, Selector> {
    let mut selector: Selector = Default::default();
    
    let (remaining, typ) = take_while(|c| is_alphanumeric(c as u8))(input)?;
    selector.typ = if typ.len() > 0 { Some(typ.into()) } else { None };

    let (remaining, pairs) = many0(alt((class, id, any)))(remaining)?;

    for pair in pairs {
        match pair {
            SelectorPart::Class(v) => selector.classes.push(v),
            SelectorPart::Id(v) => selector.id = Some(v),
            SelectorPart::Any(k, v) => if k == "name" { selector.name = Some(v); },
        }
    }

    Ok((remaining, selector))
}

fn class(input: &str) -> IResult<&str, SelectorPart> {
    preceded(char('.'), take_while(|c| is_alphanumeric(c as u8)))(input).map(|(r, v)| (r, SelectorPart::Class(v.into())))
}

fn id(input: &str) -> IResult<&str, SelectorPart> {
    preceded(char('#'), take_while(|c| is_alphanumeric(c as u8)))(input).map(|(r, v)| (r, SelectorPart::Id(v.into())))
}

fn any(input: &str) -> IResult<&str, SelectorPart> {
    let (remaining, _) = char('[')(input)?;
    let (remaining, name) = take_while(|c| is_alphanumeric(c as u8))(remaining)?;
    let (remaining, _) = char('=')(remaining)?;
    let (remaining, value) = take_while(|c| is_alphanumeric(c as u8))(remaining)?;
    let (remaining, _) = char(']')(remaining)?;
    Ok((remaining, SelectorPart::Any(name.into(), value.into())))
}

fn body(input: &str) -> IResult<&str, std::collections::HashMap::<String, CSSValue>> {
    let mut hm = std::collections::HashMap::new();
    many0(kv)(input).map(|v| { v.1.into_iter().for_each(|v| { hm.insert(v.0.into(), v.1); }); (v.0, hm) })
}

fn kv(input: &str) -> IResult<&str, (&str, CSSValue)> {
    let (remaining, (kv, _)) = tuple((
        separated_pair(css_name, char(':'), css_value),
        char(';')
    ))(input)?;
    Ok((remaining, kv))
}

fn css_name(input: &str) -> IResult<&str, &str> {
    whitespace(take_while(|c| is_alphanumeric(c as u8) || c == '-'))(input)
}

fn css_value(input: &str) -> IResult<&str, CSSValue> {
    alt((
        whitespace(hex_color),
        string,
    ))(input)
}

#[derive(Debug, Clone)]
pub enum CSSValue {
    String(String),
    Color(Color),
}

fn string(input: &str) -> IResult<&str, CSSValue> {
  let (input, value) = whitespace(take_while(|c| is_alphanumeric(c as u8) || c == '-'))(input)?;

  Ok((input, CSSValue::String(value.into())))
}

#[derive(Debug,PartialEq, Clone)]
pub struct Color {
  pub r:   u8,
  pub g: u8,
  pub b:  u8,
}

fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
  u8::from_str_radix(input, 16)
}

fn is_hex_digit(c: char) -> bool {
  c.is_digit(16)
}

fn hex_primary(input: &str) -> IResult<&str, u8> {
  map_res(
    take_while_m_n(2, 2, is_hex_digit),
    from_hex
  )(input)
}

fn hex_color(input: &str) -> IResult<&str, CSSValue> {
  let (input, _) = tag("#")(input)?;
  let (input, (r, g, b)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;

  Ok((input, CSSValue::Color(Color { r, g, b })))
}