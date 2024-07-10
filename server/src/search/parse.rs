pub fn post_query(query: &str) {
    let terms = query.split_whitespace().collect::<Vec<_>>();

    for mut term in terms.into_iter() {
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

#[cfg(test)]
mod test {}
