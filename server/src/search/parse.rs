use crate::search::Error;
use crate::search::{Criteria, StrCritera, TimeParsingError};
use crate::util::DateTime;
use std::str::FromStr;

pub fn str_criteria(filter: &str) -> StrCritera {
    if filter.contains('*') {
        StrCritera::WildCard(filter.replace('*', "%"))
    } else {
        StrCritera::Regular(parse_regular_str(filter))
    }
}

pub fn time_criteria(filter: &str) -> Result<Criteria<DateTime>, Error> {
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

pub fn criteria<T>(filter: &str) -> Result<Criteria<T>, Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error,
    <T as FromStr>::Err: 'static,
{
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => Ok(Criteria::GreaterEq(left.parse().map_err(Box::from)?)),
            ("", right) => Ok(Criteria::LessEq(right.parse().map_err(Box::from)?)),
            (left, right) => Ok(Criteria::Range(left.parse().map_err(Box::from)?..right.parse().map_err(Box::from)?)),
        };
    }
    filter
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map(Criteria::Values)
        .map_err(Box::from)
        .map_err(Error::from)
}

fn parse_time(time: &str) -> Result<DateTime, TimeParsingError> {
    if time == "today" {
        return Ok(DateTime::today());
    } else if time == "yesterday" {
        return Ok(DateTime::yesterday());
    }

    let mut date_iterator = time.split('-');
    let year: i32 = date_iterator
        .next()
        .ok_or(TimeParsingError::TooFewArgs)
        .and_then(|value| value.parse().map_err(TimeParsingError::from))?;
    let month: Option<u8> = date_iterator.next().map(|value| value.parse()).transpose()?;
    let day: Option<u8> = date_iterator.next().map(|value| value.parse()).transpose()?;
    if date_iterator.next().is_some() {
        return Err(TimeParsingError::TooManyArgs);
    }

    DateTime::from_date(year, month.unwrap_or(1), day.unwrap_or(1)).map_err(TimeParsingError::from)
}

fn parse_regular_str(filter: &str) -> Criteria<&str> {
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => Criteria::GreaterEq(left),
            ("", right) => Criteria::LessEq(right),
            (left, right) => Criteria::Range(left..right),
        };
    }
    Criteria::Values(filter.split(',').collect())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::enums::PostSafety;

    #[test]
    fn time_parsing() {
        assert_eq!(parse_time("1970").unwrap(), DateTime::from_date(1970, 1, 1).unwrap());
        assert_eq!(parse_time("2024").unwrap(), DateTime::from_date(2024, 1, 1).unwrap());
        assert_eq!(parse_time("2024-7").unwrap(), DateTime::from_date(2024, 7, 1).unwrap());
        assert_eq!(parse_time("2024-7-11").unwrap(), DateTime::from_date(2024, 7, 11).unwrap());
        assert_eq!(parse_time("2024-07-11").unwrap(), DateTime::from_date(2024, 7, 11).unwrap());
        assert_eq!(parse_time("2024-2-29").unwrap(), DateTime::from_date(2024, 2, 29).unwrap());

        assert!(parse_time("").is_err());
        assert!(parse_time("2000-01-01-01").is_err());
        assert!(parse_time("2025-2-29").is_err());
        assert!(parse_time("Hello World!").is_err());
        assert!(parse_time("a-b-c").is_err());
        assert!(parse_time("--").is_err());
    }

    #[test]
    fn criteria_parsing() {
        assert_eq!(criteria("1").unwrap(), Criteria::Values(vec![1]));
        assert_eq!(criteria("-137").unwrap(), Criteria::Values(vec![-137]));
        assert_eq!(criteria("0,1,2,3").unwrap(), Criteria::Values(vec![0, 1, 2, 3]));
        assert_eq!(criteria("-4,-1,0,0,70,6").unwrap(), Criteria::Values(vec![-4, -1, 0, 0, 70, 6]));
        assert_eq!(criteria("7..").unwrap(), Criteria::GreaterEq(7));
        assert_eq!(criteria("..7").unwrap(), Criteria::LessEq(7));
        assert_eq!(criteria("0..1").unwrap(), Criteria::Range(0..1));
        assert_eq!(criteria("-10..5").unwrap(), Criteria::Range(-10..5));

        assert_eq!(str_criteria("str"), StrCritera::Regular(Criteria::Values(vec!["str"])));
        assert_eq!(str_criteria("a,b,c"), StrCritera::Regular(Criteria::Values(vec!["a", "b", "c"])));
        assert_eq!(str_criteria("a.."), StrCritera::Regular(Criteria::GreaterEq("a")));
        assert_eq!(str_criteria("..z"), StrCritera::Regular(Criteria::LessEq("z")));
        assert_eq!(str_criteria("a..z"), StrCritera::Regular(Criteria::Range("a".."z")));
        assert_eq!(str_criteria("*str*"), StrCritera::WildCard(String::from("%str%")));
        assert_eq!(str_criteria("a*,*b,*c*"), StrCritera::WildCard(String::from("a%,%b,%c%")));
        assert_eq!(str_criteria("*a..b"), StrCritera::WildCard(String::from("%a..b")));

        assert_eq!(criteria("safe").unwrap(), Criteria::Values(vec![PostSafety::Safe]));
        assert_eq!(criteria("safe..unsafe").unwrap(), Criteria::Range(PostSafety::Safe..PostSafety::Unsafe));
    }
}
