use crate::api::error::{ApiError, ApiResult};
use crate::search::{Condition, StrCondition, TimeParsingError};
use crate::time::DateTime;
use std::ops::Range;
use std::str::FromStr;
use time::{Date, Duration, Month, OffsetDateTime, Time};

/// NOTE: This can be replaced by official Pattern trait when stabilized.
pub trait Pattern: Copy {
    fn matches(self, c: char) -> bool;
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

/// Like [`str::split_whitespace`], but ignores whitespace escaped with `\`.
pub fn split_unescaped_whitespace(text: &str) -> impl Iterator<Item = &str> {
    SplitUnescaped::new(text, IsWhitespace).filter(|term| !term.is_empty())
}

/// Splits `text` into two parts by an unescaped character matching given `pattern`.
pub fn split_once<P: Pattern>(text: &str, pattern: P) -> Option<(&str, &str)> {
    next_unescaped_split(text, pattern)
        .map(|index| text.split_at(index))
        .map(|(left, right)| (left, right.strip_prefix(|c| pattern.matches(c)).unwrap_or_default()))
}

/// Parses string-based `condition`.
pub fn str_condition(condition: &str) -> StrCondition {
    if next_unescaped_split(condition, '*').is_some() {
        StrCondition::WildCard(to_like_pattern(condition))
    } else {
        StrCondition::Regular(parse_regular_str(condition))
    }
}

/// Parses time-based `condition`.
pub fn time_condition(condition: &str) -> ApiResult<Condition<Range<DateTime>>> {
    if let Some(split_str) = condition.split_once("..") {
        return match split_str {
            (left, "") => parse_time(left).map(Condition::GreaterEq).map_err(ApiError::from),
            ("", right) => parse_time(right).map(Condition::LessEq).map_err(ApiError::from),
            (left, right) => Ok(Condition::Range(parse_time(left)?..parse_time(right)?)),
        };
    }
    condition
        .split(',')
        .map(parse_time)
        .collect::<Result<_, _>>()
        .map(Condition::Values)
        .map_err(ApiError::from)
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
        .map_err(ApiError::from)
}

// Checks if condition represents multiple values (i.e. a list, range, or wildcard pattern).
pub fn is_multivalued(condition: &str) -> bool {
    next_unescaped_split(condition, ',').is_some()
        || next_unescaped_split(condition, '*').is_some()
        || range_split(condition).is_some()
}

#[derive(Clone, Copy, PartialEq)]
enum CharKind {
    Normal,  // Unquoted, unescaped — splittable, `*` is a wildcard
    Literal, // Inside quotes or after `\` — never split, never wildcard
    Quote,   // An unescaped `"` — syntax, dropped on unescape
    Escape,  // An unescaped `\` — syntax, dropped on unescape
}

/// A general iterator over patterns that are not escaped with `\`.
struct SplitUnescaped<'a, P> {
    text: &'a str,
    start: usize,
    pattern: P,
}

impl<'a, P: Pattern> SplitUnescaped<'a, P> {
    fn new(text: &'a str, pattern: P) -> Self {
        Self {
            text,
            start: 0,
            pattern,
        }
    }
}

impl<'a, P: Pattern> Iterator for SplitUnescaped<'a, P> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        let remainder = self.text.get(self.start..)?;
        if remainder.is_empty() {
            return None;
        }

        if let Some(index) = next_unescaped_split(remainder, self.pattern) {
            // Advance past the delimiter by its actual byte width
            let delimiter_len = remainder[index..].chars().next().map_or(1, char::len_utf8);
            self.start += index + delimiter_len;
            Some(&remainder[..index])
        } else {
            self.start = self.text.len() + 1; // Terminate
            Some(remainder)
        }
    }
}

fn scan(text: &str) -> impl Iterator<Item = (usize, char, CharKind)> + '_ {
    let mut quoted = false;
    let mut escaped = false;
    text.char_indices().map(move |(i, c)| {
        let kind = if escaped {
            escaped = false;
            CharKind::Literal
        } else if c == '\\' {
            escaped = true;
            CharKind::Escape
        } else if c == '"' {
            quoted = !quoted;
            CharKind::Quote
        } else if quoted {
            CharKind::Literal
        } else {
            CharKind::Normal
        };
        (i, c, kind)
    })
}

/// Finds the index of next unescaped character that matches `pattern` in `text`.
fn next_unescaped_split<P: Pattern>(text: &str, pattern: P) -> Option<usize> {
    scan(text).find_map(|(i, c, kind)| (kind == CharKind::Normal && pattern.matches(c)).then_some(i))
}

/// Splits `text` into two parts if it contains an unescaped ".." substring.
fn range_split(text: &str) -> Option<(&str, &str)> {
    scan(text)
        .find_map(|(i, _, kind)| (kind == CharKind::Normal && text[i..].starts_with("..")).then_some(i))
        .map(|i| (&text[..i], &text[i + 2..]))
}

/// Replaces escaped characters with unescaped ones in `text`.
fn unescape(text: &str) -> String {
    scan(text)
        .filter_map(|(_, c, kind)| matches!(kind, CharKind::Normal | CharKind::Literal).then_some(c))
        .collect()
}

/// Converts a wildcard query into a SQL LIKE pattern.
/// Unescaped `*` becomes `%`, literal `\`, `%`, and `_` are LIKE-escaped.
fn to_like_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len());
    for (_, c, kind) in scan(text) {
        match kind {
            CharKind::Quote | CharKind::Escape => {}
            CharKind::Normal if c == '*' => pattern.push('%'),
            CharKind::Normal | CharKind::Literal => {
                if matches!(c, '\\' | '%' | '_') {
                    pattern.push('\\');
                }
                pattern.push(c);
            }
        }
    }
    pattern
}

/// Parses a non-wildcard string-based `filter`.
fn parse_regular_str(filter: &str) -> Condition<String> {
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
        return Ok(DateTime::today_utc()..DateTime::tomorrow_utc());
    } else if time == "yesterday" {
        return Ok(DateTime::yesterday_utc()..DateTime::today_utc());
    }

    let mut date_iterator = time.split('-');
    let year: i32 = date_iterator.next().unwrap_or("").parse()?;
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

/// Tries to parse `text` into a [`Month`]. Returns [`TimeParsingError`] on failure.
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
        assert_eq!(str_condition("*str*"), StrCondition::WildCard("%str%".into()));
        assert_eq!(str_condition("a*,*b,*c*"), StrCondition::WildCard("a%,%b,%c%".into()));
        assert_eq!(str_condition("*a..b"), StrCondition::WildCard("%a..b".into()));

        assert_eq!(condition("safe")?, Condition::Values(vec![PostSafety::Safe]));
        assert_eq!(condition("safe..unsafe")?, Condition::Range(PostSafety::Safe..PostSafety::Unsafe));
        Ok(())
    }

    #[test]
    fn condition_parsing_multibyte_chars() {
        assert_eq!(split_once("café:bar", ':'), Some(("café", "bar")));
        assert_eq!(split_once(r"é\:b", ':'), None);
        assert_eq!(split_once("a※b", '※'), Some(("a", "b")));
        assert_eq!(split_once(r"a\:é", ':'), None);

        assert_eq!(
            str_condition("🦀,日本語,émoji"),
            StrCondition::Regular(Condition::Values(vec!["🦀".into(), "日本語".into(), "émoji".into()]))
        );
        assert_eq!(str_condition(r"a\,é"), StrCondition::Regular(Condition::Values(vec!["a,é".into()])));
        assert_eq!(str_condition(r"a\..é"), StrCondition::Regular(Condition::Values(vec!["a..é".into()])));

        assert_eq!(str_condition("é.."), StrCondition::Regular(Condition::GreaterEq("é".into())));
        assert_eq!(str_condition("..é"), StrCondition::Regular(Condition::LessEq("é".into())));
        assert_eq!(str_condition("あ..ん"), StrCondition::Regular(Condition::Range("あ".into().."ん".into())));
        assert_eq!(str_condition("🦀..🦞"), StrCondition::Regular(Condition::Range("🦀".into().."🦞".into())));

        assert_eq!(str_condition("🦀*"), StrCondition::WildCard("🦀%".into()));
        assert_eq!(str_condition("*日本語*"), StrCondition::WildCard("%日本語%".into()));
        assert_eq!(str_condition("ÉCLAIR*"), StrCondition::WildCard("ÉCLAIR%".into()));
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

        let mut with_escapes = SplitUnescaped::new(r"The quick\ brown\ fox jumps over the lazy\ dog.", IsWhitespace);
        assert_eq!(with_escapes.next(), Some("The"));
        assert_eq!(with_escapes.next(), Some(r"quick\ brown\ fox"));
        assert_eq!(with_escapes.next(), Some("jumps"));
        assert_eq!(with_escapes.next(), Some("over"));
        assert_eq!(with_escapes.next(), Some("the"));
        assert_eq!(with_escapes.next(), Some(r"lazy\ dog."));
        assert_eq!(with_escapes.next(), None);

        let mut escaped_escapes = SplitUnescaped::new(r"lazy\\ dog.", IsWhitespace);
        assert_eq!(escaped_escapes.next(), Some(r"lazy\\"));
        assert_eq!(escaped_escapes.next(), Some("dog."));
        assert_eq!(escaped_escapes.next(), None);

        let mut many_escapes = SplitUnescaped::new(r"lazy\\\\\\\ dog.", IsWhitespace);
        assert_eq!(many_escapes.next(), Some(r"lazy\\\\\\\ dog."));
        assert_eq!(many_escapes.next(), None);
    }

    #[test]
    fn split_unescaped_multibyte_chars() {
        let mut without_escapes = SplitUnescaped::new("Ｗｉｄｅ 𝓯𝓪𝓷𝓬𝔂 Z̸̢̪̈́a̴͚̕l̶̡̎g̷̻̈o̶̸͇͝text العربية ᚛ᚉᚑᚅᚔ᚜ 𝄞 🦀", IsWhitespace);
        assert_eq!(without_escapes.next(), Some("Ｗｉｄｅ"));
        assert_eq!(without_escapes.next(), Some("𝓯𝓪𝓷𝓬𝔂"));
        assert_eq!(without_escapes.next(), Some("Z̸̢̪̈́a̴͚̕l̶̡̎g̷̻̈o̶̸͇͝text"));
        assert_eq!(without_escapes.next(), Some("العربية"));
        assert_eq!(without_escapes.next(), Some("᚛ᚉᚑᚅᚔ᚜"));
        assert_eq!(without_escapes.next(), Some("𝄞"));
        assert_eq!(without_escapes.next(), Some("🦀"));
        assert_eq!(without_escapes.next(), None);

        let mut with_escapes = SplitUnescaped::new(r"Ｗｉｄｅ\ 𝓯𝓪𝓷𝓬𝔂\ Z̸̢̪̈́a̴͚̕l̶̡̎g̷̻̈o̶̸͇͝text العربية\ ᚛ᚉᚑᚅᚔ᚜ 𝄞\ 🦀", IsWhitespace);
        assert_eq!(with_escapes.next(), Some(r"Ｗｉｄｅ\ 𝓯𝓪𝓷𝓬𝔂\ Z̸̢̪̈́a̴͚̕l̶̡̎g̷̻̈o̶̸͇͝text"));
        assert_eq!(with_escapes.next(), Some(r"العربية\ ᚛ᚉᚑᚅᚔ᚜"));
        assert_eq!(with_escapes.next(), Some(r"𝄞\ 🦀"));
        assert_eq!(with_escapes.next(), None);

        let mut escaped_escapes = SplitUnescaped::new(r"Ｗｉｄｅ\\ 𝓯𝓪𝓷𝓬𝔂\\ 𝄞\\ 🦀", IsWhitespace);
        assert_eq!(escaped_escapes.next(), Some(r"Ｗｉｄｅ\\"));
        assert_eq!(escaped_escapes.next(), Some(r"𝓯𝓪𝓷𝓬𝔂\\"));
        assert_eq!(escaped_escapes.next(), Some(r"𝄞\\"));
        assert_eq!(escaped_escapes.next(), Some("🦀"));
        assert_eq!(escaped_escapes.next(), None);

        let mut many_escapes = SplitUnescaped::new(r"Ｗｉｄｅ\\\\\\\ 𝓯𝓪𝓷𝓬𝔂.", IsWhitespace);
        assert_eq!(many_escapes.next(), Some(r"Ｗｉｄｅ\\\\\\\ 𝓯𝓪𝓷𝓬𝔂."));
        assert_eq!(many_escapes.next(), None);
    }

    #[test]
    fn split_unescape_multibyte_delimiter() {
        let mut multibyte_delimiter = SplitUnescaped::new("a※b※c※É※𝄞※🦀", '※');
        assert_eq!(multibyte_delimiter.next(), Some("a"));
        assert_eq!(multibyte_delimiter.next(), Some("b"));
        assert_eq!(multibyte_delimiter.next(), Some("c"));
        assert_eq!(multibyte_delimiter.next(), Some("É"));
        assert_eq!(multibyte_delimiter.next(), Some("𝄞"));
        assert_eq!(multibyte_delimiter.next(), Some("🦀"));
        assert_eq!(multibyte_delimiter.next(), None);

        let mut multibyte_delimiter_escaped = SplitUnescaped::new(r"a\※b※c※É\※𝄞※🦀", '※');
        assert_eq!(multibyte_delimiter_escaped.next(), Some(r"a\※b"));
        assert_eq!(multibyte_delimiter_escaped.next(), Some("c"));
        assert_eq!(multibyte_delimiter_escaped.next(), Some(r"É\※𝄞"));
        assert_eq!(multibyte_delimiter_escaped.next(), Some("🦀"));
        assert_eq!(multibyte_delimiter_escaped.next(), None);
    }

    #[test]
    fn split_unescaped_edge_cases() {
        let mut empty_string = SplitUnescaped::new("", IsWhitespace);
        assert_eq!(empty_string.next(), None);

        let mut only_escape = SplitUnescaped::new(r"\", IsWhitespace);
        assert_eq!(only_escape.next(), Some(r"\"));
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
        assert_eq!(split_once(r"a\:b", ':'), None);
        assert_eq!(split_once(r"a\::b", ':'), Some((r"a\:", "b")));
        assert_eq!(split_once(r"a\\:b", ':'), Some((r"a\\", "b")));

        assert_eq!(unescape(r"\."), ".".to_owned());
        assert_eq!(unescape(r"\\."), r"\.".to_owned());
        assert_eq!(unescape(r"\\\."), r"\.".to_owned());
        assert_eq!(unescape(r"\\\\."), r"\\.".to_owned());
        assert_eq!(unescape(r",\.,x.\\:.j..\,"), r",.,x.\:.j..,".to_owned());

        // Check that escaped tokens are escaped properly
        assert_eq!(
            str_condition(r"a\,,b\.,c\:,d\\,e"),
            StrCondition::Regular(Condition::Values(vec![
                "a,".into(),
                "b.".into(),
                "c:".into(),
                r"d\".into(),
                "e".into()
            ]))
        );

        // Commas with an even number of backslashes behind it shouldn't be escaped
        assert_eq!(
            str_condition(r"a\\,b\\\,c\\\\,d"),
            StrCondition::Regular(Condition::Values(vec![r"a\".into(), r"b\,c\\".into(), "d".into()]))
        );

        // Check that ranged conditions are escaped properly
        assert_eq!(str_condition(r"a\..b"), StrCondition::Regular(Condition::Values(vec!["a..b".into()])));
        assert_eq!(str_condition(r"a.\.b"), StrCondition::Regular(Condition::Values(vec!["a..b".into()])));
        assert_eq!(str_condition(r"a\\..b"), StrCondition::Regular(Condition::Range(r"a\".into().."b".into())));
        assert_eq!(str_condition(r"\..."), StrCondition::Regular(Condition::GreaterEq(".".into())));
        assert_eq!(str_condition(r"..\."), StrCondition::Regular(Condition::LessEq(".".into())));
        assert_eq!(str_condition(r"\\..."), StrCondition::Regular(Condition::Range(r"\".into()..".".into())));
    }

    #[test]
    fn escaped_asterisk() {
        assert_eq!(str_condition(r"a\*b"), StrCondition::Regular(Condition::Values(vec!["a*b".into()])));
        assert_eq!(str_condition(r"a\*b*"), StrCondition::WildCard("a*b%".into()));
        assert_eq!(str_condition(r"a\\*b"), StrCondition::WildCard(r"a\\%b".into()));
        assert_eq!(str_condition(r"a\\\*b"), StrCondition::Regular(Condition::Values(vec![r"a\*b".into()])));
        assert_eq!(str_condition(r"a\\\\*b"), StrCondition::WildCard(r"a\\\\%b".into()));
    }

    #[test]
    fn pattern_escaping() {
        assert_eq!(str_condition("a_b*"), StrCondition::WildCard(r"a\_b%".into()));
        assert_eq!(str_condition(r"a\_b*"), StrCondition::WildCard(r"a\_b%".into()));
        assert_eq!(str_condition(r"a\\_b*"), StrCondition::WildCard(r"a\\\_b%".into()));
        assert_eq!(str_condition("100%*"), StrCondition::WildCard(r"100\%%".into()));
        assert_eq!(str_condition(r"100\%*"), StrCondition::WildCard(r"100\%%".into()));
        assert_eq!(str_condition(r"100\\%*"), StrCondition::WildCard(r"100\\\%%".into()));
    }

    #[test]
    fn string_literals() {
        assert_eq!(split_once(r#""a:b":c"#, ':'), Some((r#""a:b""#, "c")));
        assert_eq!(split_once(r#""a:b""#, ':'), None);
        assert_eq!(split_once(r#"a":"※b"#, '※'), Some((r#"a":""#, "b")));

        // Quoted delimiters are treated as literals
        assert_eq!(str_condition(r#""a,b""#), StrCondition::Regular(Condition::Values(vec!["a,b".into()])));
        assert_eq!(str_condition(r#""a..b""#), StrCondition::Regular(Condition::Values(vec!["a..b".into()])));
        assert_eq!(str_condition(r#""a*b""#), StrCondition::Regular(Condition::Values(vec!["a*b".into()])));

        // Quotes are stripped from plain values
        assert_eq!(str_condition(r#""str""#), StrCondition::Regular(Condition::Values(vec!["str".into()])));
        assert_eq!(str_condition(r#""""#), StrCondition::Regular(Condition::Values(vec!["".into()])));

        // Quoted sections can be mixed with unquoted text
        assert_eq!(str_condition(r#"a"b,c"d"#), StrCondition::Regular(Condition::Values(vec!["ab,cd".into()])));
        assert_eq!(
            str_condition(r#""a,b",c,"d""#),
            StrCondition::Regular(Condition::Values(vec!["a,b".into(), "c".into(), "d".into()]))
        );

        // Delimiters outside quotes still split
        assert_eq!(str_condition(r#""a","b""#), StrCondition::Regular(Condition::Values(vec!["a".into(), "b".into()])));
        assert_eq!(str_condition(r#""a".."b""#), StrCondition::Regular(Condition::Range("a".into().."b".into())));
        assert_eq!(str_condition(r#""a,b".."#), StrCondition::Regular(Condition::GreaterEq("a,b".into())));
        assert_eq!(str_condition(r#".."y,z""#), StrCondition::Regular(Condition::LessEq("y,z".into())));

        // Wildcards outside quotes are still wildcards
        assert_eq!(str_condition(r#""a*b"*"#), StrCondition::WildCard("a*b%".into()));
        assert_eq!(str_condition(r#""100%"*"#), StrCondition::WildCard(r"100\%%".into()));
        assert_eq!(str_condition(r#""a_b"*"#), StrCondition::WildCard(r"a\_b%".into()));
    }

    #[test]
    fn string_literal_escapes() {
        // Escaped quotes are literal quotes
        assert_eq!(unescape(r#"\""#), r#"""#.to_owned());
        assert_eq!(str_condition(r#"a\"b"#), StrCondition::Regular(Condition::Values(vec![r#"a"b"#.into()])));
        assert_eq!(
            str_condition(r#""say \"hi\"""#),
            StrCondition::Regular(Condition::Values(vec![r#"say "hi""#.into()]))
        );

        // Escaped quotes do not open or close a literal
        assert_eq!(
            str_condition(r#"a\",b"#),
            StrCondition::Regular(Condition::Values(vec![r#"a""#.into(), "b".into()]))
        );
        assert_eq!(str_condition(r#""a\",b""#), StrCondition::Regular(Condition::Values(vec![r#"a",b"#.into()])));

        // Backslashes with even parity before a quote leave it unescaped
        assert_eq!(str_condition(r#"a\\",b""#), StrCondition::Regular(Condition::Values(vec![r"a\,b".into()])));
    }

    #[test]
    fn split_unescaped_string_literals() {
        let mut quoted_whitespace = SplitUnescaped::new(r#"The "quick brown fox" jumps"#, IsWhitespace);
        assert_eq!(quoted_whitespace.next(), Some("The"));
        assert_eq!(quoted_whitespace.next(), Some(r#""quick brown fox""#));
        assert_eq!(quoted_whitespace.next(), Some("jumps"));
        assert_eq!(quoted_whitespace.next(), None);

        let mut quoted_delimiter = SplitUnescaped::new(r#"a,"b,c",d"#, ',');
        assert_eq!(quoted_delimiter.next(), Some("a"));
        assert_eq!(quoted_delimiter.next(), Some(r#""b,c""#));
        assert_eq!(quoted_delimiter.next(), Some("d"));
        assert_eq!(quoted_delimiter.next(), None);

        let mut escaped_quote = SplitUnescaped::new(r#"a\" b"c d""#, IsWhitespace);
        assert_eq!(escaped_quote.next(), Some(r#"a\""#));
        assert_eq!(escaped_quote.next(), Some(r#"b"c d""#));
        assert_eq!(escaped_quote.next(), None);
    }

    #[test]
    fn string_literal_multibyte_chars() {
        assert_eq!(str_condition(r#""日本,語""#), StrCondition::Regular(Condition::Values(vec!["日本,語".into()])));
        assert_eq!(str_condition(r#""🦀..🦞""#), StrCondition::Regular(Condition::Values(vec!["🦀..🦞".into()])));
        assert_eq!(str_condition(r#"é"※,※"é"#), StrCondition::Regular(Condition::Values(vec!["é※,※é".into()])));
        assert_eq!(str_condition(r#""𝄞 *"*"#), StrCondition::WildCard("𝄞 *%".into()));
    }

    #[test]
    fn string_literal_edge_cases() {
        // A quoted asterisk alone never creates a wildcard
        assert_eq!(str_condition(r#""*""#), StrCondition::Regular(Condition::Values(vec!["*".into()])));

        // Empty quoted values in lists and ranges
        assert_eq!(
            str_condition(r#"a,"",b"#),
            StrCondition::Regular(Condition::Values(vec!["a".into(), "".into(), "b".into()]))
        );
        assert_eq!(str_condition(r#"""..b"#), StrCondition::Regular(Condition::Range("".into().."b".into())));

        // Quotes adjacent to range dots
        assert_eq!(str_condition(r#"a"."..b"#), StrCondition::Regular(Condition::Range("a.".into().."b".into())));
        assert_eq!(str_condition(r#"a.."."b"#), StrCondition::Regular(Condition::Range("a".into()..".b".into())));

        // A quoted dot followed by a real dot is not a range
        assert_eq!(str_condition(r#"a".".b"#), StrCondition::Regular(Condition::Values(vec!["a..b".into()])));

        // Escaped backslash inside quotes is a literal backslash
        assert_eq!(unescape(r#""a\\b""#), r"a\b".to_owned());
        assert_eq!(str_condition(r#""a\\,b""#), StrCondition::Regular(Condition::Values(vec![r"a\,b".into()])));

        // Backslash-escaped delimiter inside quotes stays literal
        assert_eq!(str_condition(r#""a\,b""#), StrCondition::Regular(Condition::Values(vec!["a,b".into()])));
    }

    #[test]
    fn unterminated_quotes() {
        // An unterminated quote extends to the end of the string
        assert_eq!(str_condition(r#""a,b"#), StrCondition::Regular(Condition::Values(vec!["a,b".into()])));
        assert_eq!(str_condition(r#"a"..b"#), StrCondition::Regular(Condition::Values(vec!["a..b".into()])));
        assert_eq!(
            str_condition(r#"a,"b,c"#),
            StrCondition::Regular(Condition::Values(vec!["a".into(), "b,c".into()]))
        );
        assert_eq!(str_condition(r#""a*"#), StrCondition::Regular(Condition::Values(vec!["a*".into()])));
        assert_eq!(unescape(r#"""#), "".to_owned());
    }

    #[test]
    fn multivalued() {
        // Normal strings
        assert!(!is_multivalued("a"));
        assert!(!is_multivalued("a.b.c"));
        assert!(is_multivalued("a,b,c"));
        assert!(is_multivalued("a.."));
        assert!(is_multivalued("..b"));
        assert!(is_multivalued("a..b"));
        assert!(is_multivalued("a*"));

        // Escaped delimiters
        assert!(!is_multivalued(r"a\,b\,c"));
        assert!(!is_multivalued(r"a\..b"));
        assert!(!is_multivalued(r"a\*"));
        assert!(is_multivalued(r"a\\,b\\,c"));
        assert!(is_multivalued(r"a\\..b"));
        assert!(is_multivalued(r"a\\*b"));

        // String literals
        assert!(!is_multivalued(r#""a,b""#));
        assert!(!is_multivalued(r#""a..b""#));
        assert!(!is_multivalued(r#""a*b""#));
        assert!(is_multivalued(r#""a",b"#));
        assert!(is_multivalued(r#""a"..b"#));
        assert!(is_multivalued(r#""a"*"#));

        // Escaped quotes
        assert!(is_multivalued(r#"\"a,b\""#));
        assert!(is_multivalued(r#"\"a..b\""#));
        assert!(is_multivalued(r#"\"a*\""#));
        assert!(!is_multivalued(r#""\"a,b\"""#));
        assert!(!is_multivalued(r#""\"a..b\"""#));
        assert!(!is_multivalued(r#""\"a*\"""#));

        // String literals with escaped delimeters
        assert!(!is_multivalued(r#""a\,b""#));
        assert!(!is_multivalued(r#""a\..b""#));
        assert!(!is_multivalued(r#""a\*b""#));
    }
}
