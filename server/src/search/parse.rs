use crate::api::{self, ApiResult};
use crate::search::{Condition, StrCondition, TimeParsingError};
use crate::time::DateTime;
use std::borrow::Cow;
use std::ops::Range;
use std::str::FromStr;
use time::{Date, Duration, Month, OffsetDateTime, Time};

/// This can be replaced by official Pattern trait when stabilized.
pub trait Pattern: Copy {
    fn matches(self, c: char) -> bool;

    fn func(self) -> impl Fn(char) -> bool {
        move |c: char| self.matches(c)
    }
}

impl Pattern for char {
    fn matches(self, c: char) -> bool {
        self == c
    }
}

#[derive(Clone, Copy)]
pub struct IsWhitespace;

impl Pattern for IsWhitespace {
    fn matches(self, c: char) -> bool {
        c.is_whitespace()
    }
}

pub fn split_unescaped_whitespace(text: &str) -> impl Iterator<Item = &str> {
    SplitUnescaped::new(text, IsWhitespace).filter(|term| !term.is_empty())
}

/// Splits `text` into two parts by an unescaped character matching given `pattern`.
pub fn split_once<P: Pattern>(text: &str, pattern: P) -> Option<(&str, &str)> {
    next_unescaped_split(text, pattern)
        .map(|index| text.split_at(index))
        .map(|(left, right)| (left, right.strip_prefix(pattern.func()).unwrap()))
}

/// Parses string-based `condition`.
pub fn str_condition(condition: &str) -> StrCondition {
    if condition.contains('*') {
        StrCondition::WildCard(unescape(condition).replace('*', "%").replace('_', "\\_").to_lowercase())
    } else {
        StrCondition::Regular(parse_regular_str(condition))
    }
}

/// Parses time-based `condition`.
pub fn time_condition(condition: &str) -> ApiResult<Condition<Range<DateTime>>> {
    if let Some(split_str) = condition.split_once("..") {
        return match split_str {
            (left, "") => parse_time(left).map(Condition::GreaterEq).map_err(api::Error::from),
            ("", right) => parse_time(right).map(Condition::LessEq).map_err(api::Error::from),
            (left, right) => Ok(Condition::Range(parse_time(left)?..parse_time(right)?)),
        };
    }
    condition
        .split(',')
        .map(parse_time)
        .collect::<Result<_, _>>()
        .map(Condition::Values)
        .map_err(api::Error::from)
}

/// Parses a non-string non-time `condition`.
pub fn condition<T>(condition: &str) -> ApiResult<Condition<T>>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    if let Some(split_str) = condition.split_once("..") {
        return match split_str {
            (left, "") => Ok(Condition::GreaterEq(left.parse().map_err(Box::from)?)),
            ("", right) => Ok(Condition::LessEq(right.parse().map_err(Box::from)?)),
            (left, right) => Ok(Condition::Range(left.parse().map_err(Box::from)?..right.parse().map_err(Box::from)?)),
        };
    }
    values(condition).map(Condition::Values)
}

/// Parses comma-separated values.
pub fn values<T>(condition: &str) -> ApiResult<Vec<T>>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    condition
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(Box::from)
        .map_err(api::Error::from)
}

struct SplitUnescaped<'a, P> {
    text: &'a str,
    start: usize,
    end: usize,
    pattern: P,
}

impl<'a, P: Pattern> SplitUnescaped<'a, P> {
    fn new(text: &'a str, pattern: P) -> Self {
        Self {
            text,
            start: 0,
            end: next_unescaped_split(text, pattern).unwrap_or(text.len()),
            pattern,
        }
    }
}

impl<'a, P: Pattern> Iterator for SplitUnescaped<'a, P> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.text.len() {
            return None;
        }
        let next = &self.text[self.start..self.end];

        self.start = self.end + 1;
        if let Some((head, remainder)) = self.text.split_at_checked(self.start) {
            self.end = next_unescaped_split(remainder, self.pattern).unwrap_or(remainder.len()) + head.len();
        }

        Some(next)
    }
}

fn is_unescaped(text: &str, index: usize) -> bool {
    let backslash_count = text
        .chars()
        .rev()
        .skip(text.len() - index)
        .take_while(|&c| c == '\\')
        .count();
    backslash_count % 2 == 0
}

/// Finds the index of next unescaped character that matches `pattern` in `text`.
fn next_unescaped_split<P: Pattern>(text: &str, pattern: P) -> Option<usize> {
    text.char_indices()
        .filter_map(|(index, c)| pattern.matches(c).then_some(index))
        .find(|&index| is_unescaped(text, index))
}

/// Splits `text` into two parts if it contains an unescaped ".." substring.
fn range_split(text: &str) -> Option<(&str, &str)> {
    let split_index = text
        .char_indices()
        .filter(|&(_, c)| c == '.')
        .filter_map(|(index, c)| {
            let next_char = text.chars().nth(index + 1);
            (c == '.' && next_char == Some('.')).then_some(index)
        })
        .find(|&index| is_unescaped(text, index));
    split_index
        .map(|index| text.split_at(index))
        .map(|(left, right)| (left, right.strip_prefix("..").unwrap()))
}

/// Replaces escaped characters with unescaped ones in `text`.
fn unescape(text: &str) -> Cow<str> {
    if text.contains('\\') {
        let mut escaped = text.starts_with('\\');
        let start_index = usize::from(escaped);
        Cow::Owned(
            text.chars()
                .skip(start_index)
                .filter(|&c| {
                    let skip = !escaped && c == '\\';
                    escaped = skip;
                    !skip
                })
                .collect(),
        )
    } else {
        Cow::Borrowed(text)
    }
}

/// Parses a non-wildcard string-based `filter`.
fn parse_regular_str(filter: &str) -> Condition<Cow<str>> {
    match range_split(filter) {
        Some((left, "")) => Condition::GreaterEq(unescape(left)),
        Some(("", right)) => Condition::LessEq(unescape(right)),
        Some((left, right)) => Condition::Range(unescape(left)..unescape(right)),
        None => Condition::Values(SplitUnescaped::new(filter, ',').map(unescape).collect()),
    }
}

/// Parses single `time` value.
/// Time values are always implicitly treated as a range of times.
fn parse_time(time: &str) -> Result<Range<DateTime>, TimeParsingError> {
    // Handle special cases
    if time == "today" {
        return Ok(DateTime::today()..DateTime::tomorrow());
    } else if time == "yesterday" {
        return Ok(DateTime::yesterday()..DateTime::today());
    }

    let mut date_iterator = time.split('-');
    let year: i32 = date_iterator
        .next()
        .ok_or(TimeParsingError::TooFewArgs)
        .and_then(|value| value.parse().map_err(TimeParsingError::from))?;
    let month = date_iterator.next().map(parse_month).transpose()?;
    let day: Option<u8> = date_iterator.next().map(str::parse).transpose()?;
    if date_iterator.next().is_some() {
        return Err(TimeParsingError::TooManyArgs);
    }

    let start_date =
        DateTime::from_date(year, month.unwrap_or(Month::January), day.unwrap_or(1)).map_err(TimeParsingError::from)?;
    let end_date = if day.is_some() {
        start_date.saturating_add(Duration::DAY)
    } else if let Some(month) = month {
        let end_year = if month == Month::December { year + 1 } else { year };
        let next_month = Date::from_calendar_date(end_year, month.next(), 1).unwrap_or(Date::MAX);
        OffsetDateTime::new_utc(next_month, Time::MIDNIGHT)
    } else {
        let next_year = Date::from_calendar_date(year + 1, Month::January, 1).unwrap_or(Date::MAX);
        OffsetDateTime::new_utc(next_year, Time::MIDNIGHT)
    };

    Ok(start_date..end_date.into())
}

fn parse_month(text: &str) -> Result<Month, TimeParsingError> {
    let number: u8 = text.parse()?;
    number.try_into().map_err(TimeParsingError::from)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::enums::PostSafety;

    #[test]
    fn time_parsing() -> Result<(), TimeParsingError> {
        let jan = Month::January;
        let feb = Month::February;
        let mar = Month::March;
        let jul = Month::July;
        let aug = Month::August;

        assert_eq!(parse_time("1970")?, DateTime::from_date(1970, jan, 1)?..DateTime::from_date(1971, jan, 1)?);
        assert_eq!(parse_time("2024")?, DateTime::from_date(2024, jan, 1)?..DateTime::from_date(2025, jan, 1)?);
        assert_eq!(parse_time("2024-7")?, DateTime::from_date(2024, jul, 1)?..DateTime::from_date(2024, aug, 1)?);
        assert_eq!(parse_time("2024-7-11")?, DateTime::from_date(2024, jul, 11)?..DateTime::from_date(2024, jul, 12)?);
        assert_eq!(parse_time("2024-07-11")?, DateTime::from_date(2024, jul, 11)?..DateTime::from_date(2024, jul, 12)?);
        assert_eq!(parse_time("2024-2-29")?, DateTime::from_date(2024, feb, 29)?..DateTime::from_date(2024, mar, 1)?);

        assert!(parse_time("").is_err());
        assert!(parse_time("2000-01-01-01").is_err());
        assert!(parse_time("2025-2-29").is_err());
        assert!(parse_time("Hello World!").is_err());
        assert!(parse_time("a-b-c").is_err());
        assert!(parse_time("--").is_err());
        Ok(())
    }

    #[test]
    fn condition_parsing() -> ApiResult<()> {
        assert_eq!(split_once("a:b", ':'), Some(("a", "b")));
        assert_eq!(split_once(":b", ':'), Some(("", "b")));
        assert_eq!(split_once("a:", ':'), Some(("a", "")));
        assert_eq!(split_once(":", ':'), Some(("", "")));

        assert_eq!(condition("1")?, Condition::Values(vec![1]));
        assert_eq!(condition("-137")?, Condition::Values(vec![-137]));
        assert_eq!(condition("0,1,2,3")?, Condition::Values(vec![0, 1, 2, 3]));
        assert_eq!(condition("-4,-1,0,0,70,6")?, Condition::Values(vec![-4, -1, 0, 0, 70, 6]));
        assert_eq!(condition("7..")?, Condition::GreaterEq(7));
        assert_eq!(condition("..7")?, Condition::LessEq(7));
        assert_eq!(condition("0..1")?, Condition::Range(0..1));
        assert_eq!(condition("-10..5")?, Condition::Range(-10..5));

        assert_eq!(str_condition("str"), StrCondition::Regular(Condition::Values(vec!["str".into()])));
        assert_eq!(
            str_condition("a,b,c"),
            StrCondition::Regular(Condition::Values(vec!["a".into(), "b".into(), "c".into()]))
        );

        assert_eq!(str_condition("a.."), StrCondition::Regular(Condition::GreaterEq("a".into())));
        assert_eq!(str_condition("..z"), StrCondition::Regular(Condition::LessEq("z".into())));
        assert_eq!(str_condition("a..z"), StrCondition::Regular(Condition::Range("a".into().."z".into())));
        assert_eq!(str_condition("*str*"), StrCondition::WildCard(String::from("%str%")));
        assert_eq!(str_condition("a*,*b,*c*"), StrCondition::WildCard(String::from("a%,%b,%c%")));
        assert_eq!(str_condition("*a..b"), StrCondition::WildCard(String::from("%a..b")));

        assert_eq!(condition("safe")?, Condition::Values(vec![PostSafety::Safe]));
        assert_eq!(condition("safe..unsafe")?, Condition::Range(PostSafety::Safe..PostSafety::Unsafe));
        Ok(())
    }

    #[test]
    fn split_unescaped() {
        let mut without_escapes = SplitUnescaped::new("The quick brown fox jumps over the lazy dog.", IsWhitespace);
        assert_eq!(without_escapes.next(), Some("The"));
        assert_eq!(without_escapes.next(), Some("quick"));
        assert_eq!(without_escapes.next(), Some("brown"));
        assert_eq!(without_escapes.next(), Some("fox"));
        assert_eq!(without_escapes.next(), Some("jumps"));
        assert_eq!(without_escapes.next(), Some("over"));
        assert_eq!(without_escapes.next(), Some("the"));
        assert_eq!(without_escapes.next(), Some("lazy"));
        assert_eq!(without_escapes.next(), Some("dog."));
        assert_eq!(without_escapes.next(), None);

        let mut with_escapes = SplitUnescaped::new("The quick\\ brown\\ fox jumps over the lazy\\ dog.", IsWhitespace);
        assert_eq!(with_escapes.next(), Some("The"));
        assert_eq!(with_escapes.next(), Some("quick\\ brown\\ fox"));
        assert_eq!(with_escapes.next(), Some("jumps"));
        assert_eq!(with_escapes.next(), Some("over"));
        assert_eq!(with_escapes.next(), Some("the"));
        assert_eq!(with_escapes.next(), Some("lazy\\ dog."));
        assert_eq!(with_escapes.next(), None);

        let mut escaped_escapes = SplitUnescaped::new("lazy\\\\ dog.", IsWhitespace);
        assert_eq!(escaped_escapes.next(), Some("lazy\\\\"));
        assert_eq!(escaped_escapes.next(), Some("dog."));
        assert_eq!(escaped_escapes.next(), None);

        let mut many_escapes = SplitUnescaped::new("lazy\\\\\\\\\\\\\\ dog.", IsWhitespace);
        assert_eq!(many_escapes.next(), Some("lazy\\\\\\\\\\\\\\ dog."));
        assert_eq!(many_escapes.next(), None);
    }

    #[test]
    fn split_unescaped_edge_cases() {
        let mut empty_string = SplitUnescaped::new("", IsWhitespace);
        assert_eq!(empty_string.next(), None);

        let mut only_escape = SplitUnescaped::new("\\", IsWhitespace);
        assert_eq!(only_escape.next(), Some("\\"));
        assert_eq!(only_escape.next(), None);

        let mut only_delimiter = SplitUnescaped::new(",,,", ',');
        assert_eq!(only_delimiter.next(), Some(""));
        assert_eq!(only_delimiter.next(), Some(""));
        assert_eq!(only_delimiter.next(), Some(""));
        assert_eq!(only_delimiter.next(), None);

        let mut only_whitespace = SplitUnescaped::new(" \t\r\n", IsWhitespace);
        assert_eq!(only_whitespace.next(), Some(""));
        assert_eq!(only_whitespace.next(), Some(""));
        assert_eq!(only_whitespace.next(), Some(""));
        assert_eq!(only_whitespace.next(), Some(""));
        assert_eq!(only_whitespace.next(), None);
    }

    #[test]
    fn escaped_strings() {
        assert_eq!(split_once("a\\:b", ':'), None);
        assert_eq!(split_once("a\\::b", ':'), Some(("a\\:", "b")));
        assert_eq!(split_once("a\\\\:b", ':'), Some(("a\\\\", "b")));

        assert_eq!(unescape("\\."), String::from("."));
        assert_eq!(unescape("\\\\."), String::from("\\."));
        assert_eq!(unescape("\\\\\\."), String::from("\\."));
        assert_eq!(unescape("\\\\\\\\."), String::from("\\\\."));
        assert_eq!(unescape(",\\.,x.\\\\:.j..\\,"), String::from(",.,x.\\:.j..,"));

        // Check that escaped tokens are escaped properly
        assert_eq!(
            str_condition("a\\,,b\\.,c\\:,d\\\\,e"),
            StrCondition::Regular(Condition::Values(vec![
                "a,".into(),
                "b.".into(),
                "c:".into(),
                "d\\".into(),
                "e".into()
            ]))
        );

        // Commas with an even number of backslashes behind it shouldn't be escaped
        assert_eq!(
            str_condition("a\\\\,b\\\\\\,c\\\\\\\\,d"),
            StrCondition::Regular(Condition::Values(vec!["a\\".into(), "b\\,c\\\\".into(), "d".into()]))
        );

        // Check that ranged conditions are escaped properly
        assert_eq!(str_condition("a\\..b"), StrCondition::Regular(Condition::Values(vec!["a..b".into()])));
        assert_eq!(str_condition("a\\\\..b"), StrCondition::Regular(Condition::Range("a\\".into().."b".into())));
        assert_eq!(str_condition("\\..."), StrCondition::Regular(Condition::GreaterEq(".".into())));
        assert_eq!(str_condition("..\\."), StrCondition::Regular(Condition::LessEq(".".into())));
        assert_eq!(str_condition("\\\\..."), StrCondition::Regular(Condition::Range("\\".into()..".".into())));
    }
}
