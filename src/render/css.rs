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
use nom::bytes::complete::take_while;
use nom::character::complete::char;
use nom::IResult;

pub fn parse_styles(style: &str) -> Vec<Rule> {
    let (_, result) = rules(style)
        .expect("Failed to parse stylesheet.");
    result
}

#[derive(Debug)]
pub struct Rule<'a> {
    pub selector: Selector<'a>,
    pub kvs: std::collections::HashMap<&'a str, &'a str>,
}

#[derive(Debug)]
pub struct Selector<'a> {
    pub typ: Option<&'a str>,
    pub id: Option<&'a str>,
    pub classes: Vec<&'a str>,
    pub name: Option<&'a str>,
}

impl<'a> Default for Selector<'a> {
    fn default() -> Self {
        Self {
            typ: None,
            id: None,
            classes: vec![],
            name: None,
        }
    }
}

impl<'a> Selector<'a> {
    pub fn matches(&self, other: &'a Selector<'a>) -> bool {
        if let Some(t1) = other.typ {
            if let Some(t2) = self.typ {
                if t1 != t2 { return false; }
            } else {
                return false;
            }
        }

        if let Some(i1) = other.id {
            if let Some(i2) = self.id {
                if i1 != i2 { return false; }
            } else {
                return false;
            }
        }

        if let Some(n1) = other.name {
            if let Some(n2) = self.name {
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
pub enum SelectorPart<'a> {
    Class(&'a str),
    Id(&'a str),
    Any(&'a str, &'a str),
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
    preceded(char('.'), take_while(|c| is_alphanumeric(c as u8)))(input).map(|(r, v)| (r, SelectorPart::Class(v)))
}

fn id(input: &str) -> IResult<&str, SelectorPart> {
    preceded(char('#'), take_while(|c| is_alphanumeric(c as u8)))(input).map(|(r, v)| (r, SelectorPart::Id(v)))
}

fn any(input: &str) -> IResult<&str, SelectorPart> {
    let (remaining, _) = char('[')(input)?;
    let (remaining, name) = take_while(|c| is_alphanumeric(c as u8))(remaining)?;
    let (remaining, _) = char('=')(remaining)?;
    let (remaining, value) = take_while(|c| is_alphanumeric(c as u8))(remaining)?;
    let (remaining, _) = char(']')(remaining)?;
    Ok((remaining, SelectorPart::Any(name, value)))
}

fn body(input: &str) -> IResult<&str, std::collections::HashMap::<&str, &str>> {
    let mut hm = std::collections::HashMap::new();
    many0(kv)(input).map(|v| { v.1.into_iter().for_each(|v| { hm.insert(v.0, v.1); }); (v.0, hm) })
}

fn kv(input: &str) -> IResult<&str, (&str, &str)> {
    let (remaining, (kv, _)) = tuple((
        separated_pair(css_name, char(':'), css_value),
        char(';')
    ))(input)?;
    Ok((remaining, kv))
}

fn css_name(input: &str) -> IResult<&str, &str> {
    whitespace(take_while(|c| is_alphanumeric(c as u8) || c == '-'))(input)
}

fn css_value(input: &str) -> IResult<&str, &str> {
    whitespace(take_while(|c| is_alphanumeric(c as u8) || c == '-'))(input)
}