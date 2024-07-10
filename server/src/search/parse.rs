use crate::search::FilterType;
use core::fmt::Debug;
use std::str::FromStr;

pub fn post_query(query: &str) {
    for mut term in query.split_whitespace() {
        let negated = term.chars().nth(0) == Some('-');
        if negated {
            term = term.strip_prefix('-').unwrap();
        }

        match term.split_once(':') {
            Some(("sort", _value)) => unimplemented!(),
            Some(("special", _value)) => unimplemented!(),
            Some((_key, _value)) => unimplemented!(),
            None => unimplemented!(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
enum Error {
    ParseFailed(#[from] Box<dyn std::error::Error>),
}

enum NamedToken {}

fn parse_str_filter_type(filter: &str) -> FilterType<&str> {
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => FilterType::GreaterEq(left),
            ("", right) => FilterType::LessEq(right),
            (left, right) => FilterType::Range(left..right),
        };
    }
    FilterType::Values(filter.split(',').collect())
}

fn parse_filter_type<T>(filter: &str) -> Result<FilterType<T>, Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error,
    <T as FromStr>::Err: 'static,
{
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => Ok(FilterType::GreaterEq(left.parse().map_err(Box::from)?)),
            ("", right) => Ok(FilterType::LessEq(right.parse().map_err(Box::from)?)),
            (left, right) => Ok(FilterType::Range(left.parse().map_err(Box::from)?..right.parse().map_err(Box::from)?)),
        };
    }

    filter
        .split(',')
        .map(|slice| slice.parse())
        .collect::<Result<_, _>>()
        .map(FilterType::Values)
        .map_err(Box::from)
        .map_err(Error::from)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::enums::PostSafety;

    #[test]
    fn parse_filter_types() {
        assert_eq!(parse_filter_type("1").unwrap(), FilterType::Values(vec![1]));
        assert_eq!(parse_filter_type("-137").unwrap(), FilterType::Values(vec![-137]));
        assert_eq!(parse_filter_type("0,1,2,3").unwrap(), FilterType::Values(vec![0, 1, 2, 3]));
        assert_eq!(parse_filter_type("-4,-1,0,0,70,6").unwrap(), FilterType::Values(vec![-4, -1, 0, 0, 70, 6]));
        assert_eq!(parse_filter_type("7..").unwrap(), FilterType::GreaterEq(7));
        assert_eq!(parse_filter_type("..7").unwrap(), FilterType::LessEq(7));
        assert_eq!(parse_filter_type("0..1").unwrap(), FilterType::Range(0..1));
        assert_eq!(parse_filter_type("-10..5").unwrap(), FilterType::Range(-10..5));

        assert_eq!(parse_str_filter_type("str"), FilterType::Values(vec!["str"]));
        assert_eq!(parse_str_filter_type("a,b,c"), FilterType::Values(vec!["a", "b", "c"]));
        assert_eq!(parse_str_filter_type("a.."), FilterType::GreaterEq("a"));
        assert_eq!(parse_str_filter_type("..z"), FilterType::LessEq("z"));
        assert_eq!(parse_str_filter_type("a..z"), FilterType::Range("a".."z"));

        assert_eq!(parse_filter_type("safe").unwrap(), FilterType::Values(vec![PostSafety::Safe]));
        assert_eq!(parse_filter_type("safe..unsafe").unwrap(), FilterType::Range(PostSafety::Safe..PostSafety::Unsafe));
    }
}
