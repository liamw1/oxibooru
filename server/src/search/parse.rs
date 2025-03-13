use crate::search::Error;
use crate::search::{Criteria, StrCritera, TimeParsingError};
use crate::time::DateTime;
use std::borrow::Cow;
use std::ops::Range;
use std::str::FromStr;
use time::{Date, Duration, Month, OffsetDateTime, Time};

/// Splits `text` into two parts by an unescaped `delimiter`.
pub fn split_once(text: &str, delimiter: char) -> Option<(&str, &str)> {
    next_split(text, delimiter)
        .map(|index| text.split_at(index))
        .map(|(left, right)| (left, right.strip_prefix(delimiter).unwrap()))
}

/// Parses string-based `filter`.
pub fn str_criteria(filter: &str) -> StrCritera {
    if filter.contains('*') {
        StrCritera::WildCard(unescape(filter).replace('*', "%").replace('_', "\\_"))
    } else {
        StrCritera::Regular(parse_regular_str(filter))
    }
}

/// Parses time-based `filter`.
pub fn time_criteria(filter: &str) -> Result<Criteria<Range<DateTime>>, Error> {
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => parse_time(left).map(Criteria::GreaterEq).map_err(Error::from),
            ("", right) => parse_time(right).map(Criteria::LessEq).map_err(Error::from),
            (left, right) => Ok(Criteria::Range(parse_time(left)?..parse_time(right)?)),
        };
    }
    filter
        .split(',')
        .map(parse_time)
        .collect::<Result<_, _>>()
        .map(Criteria::Values)
        .map_err(Error::from)
}

/// Parses a non-string non-time `filter`.
pub fn criteria<T>(filter: &str) -> Result<Criteria<T>, Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + 'static,
{
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => Ok(Criteria::GreaterEq(left.parse().map_err(Box::from)?)),
            ("", right) => Ok(Criteria::LessEq(right.parse().map_err(Box::from)?)),
            (left, right) => Ok(Criteria::Range(left.parse().map_err(Box::from)?..right.parse().map_err(Box::from)?)),
        };
    }
    values(filter).map(Criteria::Values)
}

/// Parses comma-separated values.
pub fn values<T>(filter: &str) -> Result<Vec<T>, Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + 'static,
{
    filter
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(Box::from)
        .map_err(Error::from)
}

/// Replaces escaped characters with unescaped ones in `text`.
fn unescape(text: &str) -> Cow<str> {
    if text.contains('\\') {
        let mut escaped = text.starts_with('\\');
        let start_index = if escaped { 1 } else { 0 };
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

/// Finds the index of next unescaped `delimiter`` in `text`.
fn next_split(text: &str, delimiter: char) -> Option<usize> {
    text.char_indices()
        .filter_map(|(index, c)| (c == delimiter).then_some(index))
        .find(|index| {
            let backslash_count = text
                .chars()
                .rev()
                .skip(text.len() - index)
                .take_while(|&c| c == '\\')
                .count();
            backslash_count % 2 == 0
        })
}

/// Returns a vector of escaped substrings over given `text` split by the given `delimiter`.
fn split_escaped(text: &str, delimiter: char) -> Vec<Cow<str>> {
    let mut parts = vec![text];
    while let Some(index) = next_split(parts.last().unwrap(), delimiter) {
        let (left, right) = parts.last().unwrap().split_at(index);
        *parts.last_mut().unwrap() = left;
        parts.push(right.strip_prefix(delimiter).unwrap());
    }
    parts.into_iter().map(unescape).collect()
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
        .find(|index| {
            let backslash_count = text
                .chars()
                .rev()
                .skip(text.len() - index)
                .take_while(|&c| c == '\\')
                .count();
            backslash_count % 2 == 0
        });
    split_index
        .map(|index| text.split_at(index))
        .map(|(left, right)| (left, right.strip_prefix("..").unwrap()))
}

/// Parses a non-wildcard string-based `filter`.
fn parse_regular_str(filter: &str) -> Criteria<Cow<str>> {
    match range_split(filter) {
        Some((left, "")) => Criteria::GreaterEq(unescape(left)),
        Some(("", right)) => Criteria::LessEq(unescape(right)),
        Some((left, right)) => Criteria::Range(unescape(left)..unescape(right)),
        None => Criteria::Values(split_escaped(filter, ',')),
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
    let day: Option<u8> = date_iterator.next().map(|value| value.parse()).transpose()?;
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
    fn criteria_parsing() -> Result<(), Error> {
        assert_eq!(split_once("a:b", ':'), Some(("a", "b")));
        assert_eq!(split_once(":b", ':'), Some(("", "b")));
        assert_eq!(split_once("a:", ':'), Some(("a", "")));
        assert_eq!(split_once(":", ':'), Some(("", "")));

        assert_eq!(criteria("1")?, Criteria::Values(vec![1]));
        assert_eq!(criteria("-137")?, Criteria::Values(vec![-137]));
        assert_eq!(criteria("0,1,2,3")?, Criteria::Values(vec![0, 1, 2, 3]));
        assert_eq!(criteria("-4,-1,0,0,70,6")?, Criteria::Values(vec![-4, -1, 0, 0, 70, 6]));
        assert_eq!(criteria("7..")?, Criteria::GreaterEq(7));
        assert_eq!(criteria("..7")?, Criteria::LessEq(7));
        assert_eq!(criteria("0..1")?, Criteria::Range(0..1));
        assert_eq!(criteria("-10..5")?, Criteria::Range(-10..5));

        assert_eq!(str_criteria("str"), StrCritera::Regular(Criteria::Values(vec![Cow::Borrowed("str")])));
        assert_eq!(
            str_criteria("a,b,c"),
            StrCritera::Regular(Criteria::Values(vec![Cow::Borrowed("a"), Cow::Borrowed("b"), Cow::Borrowed("c")]))
        );

        assert_eq!(str_criteria("a.."), StrCritera::Regular(Criteria::GreaterEq(Cow::Borrowed("a"))));
        assert_eq!(str_criteria("..z"), StrCritera::Regular(Criteria::LessEq(Cow::Borrowed("z"))));
        assert_eq!(str_criteria("a..z"), StrCritera::Regular(Criteria::Range(Cow::Borrowed("a")..Cow::Borrowed("z"))));
        assert_eq!(str_criteria("*str*"), StrCritera::WildCard(String::from("%str%")));
        assert_eq!(str_criteria("a*,*b,*c*"), StrCritera::WildCard(String::from("a%,%b,%c%")));
        assert_eq!(str_criteria("*a..b"), StrCritera::WildCard(String::from("%a..b")));

        assert_eq!(criteria("safe")?, Criteria::Values(vec![PostSafety::Safe]));
        assert_eq!(criteria("safe..unsafe")?, Criteria::Range(PostSafety::Safe..PostSafety::Unsafe));
        Ok(())
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
            str_criteria("a\\,,b\\.,c\\:,d\\\\,e"),
            StrCritera::Regular(Criteria::Values(vec![
                Cow::Borrowed("a,"),
                Cow::Borrowed("b."),
                Cow::Borrowed("c:"),
                Cow::Borrowed("d\\"),
                Cow::Borrowed("e")
            ]))
        );

        // Commas with an even number of backslashes behind it shouldn't be escaped
        assert_eq!(
            str_criteria("a\\\\,b\\\\\\,c\\\\\\\\,d"),
            StrCritera::Regular(Criteria::Values(vec![
                Cow::Borrowed("a\\"),
                Cow::Borrowed("b\\,c\\\\"),
                Cow::Borrowed("d")
            ]))
        );

        // Check that ranged criterias are escaped properly
        assert_eq!(str_criteria("a\\..b"), StrCritera::Regular(Criteria::Values(vec![Cow::Borrowed("a..b")])));
        assert_eq!(
            str_criteria("a\\\\..b"),
            StrCritera::Regular(Criteria::Range(Cow::Borrowed("a\\")..Cow::Borrowed("b")))
        );
        assert_eq!(str_criteria("\\..."), StrCritera::Regular(Criteria::GreaterEq(Cow::Borrowed("."))));
        assert_eq!(str_criteria("..\\."), StrCritera::Regular(Criteria::LessEq(Cow::Borrowed("."))));
        assert_eq!(
            str_criteria("\\\\..."),
            StrCritera::Regular(Criteria::Range(Cow::Borrowed("\\")..Cow::Borrowed(".")))
        );
    }
}
