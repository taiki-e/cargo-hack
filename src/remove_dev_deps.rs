use std::cmp;

// TODO: Fix parsing of quoted keys

// Note: The input must be a valid TOML.
pub(crate) fn remove_dev_deps(text: &str) -> String {
    const DEV_DEPS: &str = "dev-dependencies";
    const TARGET: &str = "target.";
    const LN: char = '\n';

    let mut text = text.to_string();
    let mut prev = 0;
    let mut next = text.find('[');

    'outer: while let Some(mut pos) = next {
        prev = text[prev..pos].rfind(LN).map_or(prev, |n| cmp::min(n + prev + 1, pos));

        // skip '# [...' and 'foo = [...'
        if text[prev..pos].trim().is_empty() {
            let slice = text[pos + 1..].trim_start();
            if slice.starts_with(DEV_DEPS) {
                let maybe_close = pos + DEV_DEPS.len();
                match slice[DEV_DEPS.len()..].trim_start().as_bytes()[0] {
                    b'.' | b']' => {
                        for (i, _) in text[maybe_close..].match_indices('[') {
                            let back = text[maybe_close..maybe_close + i]
                                .rfind(LN)
                                .map_or(0, |n| cmp::min(n + 1, i));

                            // skip '# [...' and 'foo = [...'
                            if text[maybe_close + back..maybe_close + i].trim().is_empty() {
                                text.drain(prev..maybe_close + back);
                                next = Some(prev + i - back);
                                continue 'outer;
                            }
                        }

                        text.drain(prev..);
                        break;
                    }
                    _ => {}
                }
            } else if slice.starts_with(TARGET) {
                if let Some(close) =
                    text[pos + TARGET.len()..].find(']').map(|c| c + pos + TARGET.len())
                {
                    let mut split = text[pos..close].split('.');
                    let _ = split.next(); // `target`
                    let _ = split.next(); // `'cfg(...)'`
                    if let Some(deps) = split.next() {
                        if deps.trim() == DEV_DEPS {
                            for (i, _) in text[close..].match_indices('[') {
                                let back = text[close..close + i]
                                    .rfind(LN)
                                    .map_or(0, |n| cmp::min(n + 1, i));

                                // skip '# [...' and 'foo = [...'
                                if text[close + back..close + i].trim().is_empty() {
                                    text.drain(prev..close + back);
                                    next = Some(prev + i - back);
                                    continue 'outer;
                                }
                            }

                            text.drain(prev..);
                            break;
                        }
                    }

                    prev = pos;
                    next = text[close..].find('[').map(|n| close + n);
                    continue;
                }
            }
        }

        prev = pos;
        // pos + 0: '['
        // pos + 1: part of table name (or '[')
        // pos + 2: ']' (or part of table name)
        // pos + 3: '\n' or eof (or part of table name or ']')
        // pos + 4: start of next table or eof (or part of this table)
        pos += 4;
        next = text.get(pos..).and_then(|s| s.find('[')).map(|n| pos + n);
        continue;
    }

    text
}

#[cfg(test)]
mod tests {
    use super::remove_dev_deps;

    macro_rules! test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let input = remove_dev_deps($input);
                assert_eq!($expected, input);
            }
        };
    }

    test!(
        a,
        "\
[package]
[dependencies]
[[example]]
[dev-dependencies.opencl]
[dev-dependencies]",
        "\
[package]
[dependencies]
[[example]]
"
    );

    test!(
        b,
        "\
[package]
[dependencies]
[[example]]
[dev-dependencies.opencl]
[dev-dependencies]
",
        "\
[package]
[dependencies]
[[example]]
"
    );

    test!(
        c,
        "\
[dev-dependencies]
foo = { features = [] }
bar = \"0.1\"
",
        "\
         "
    );

    test!(
        d,
        "\
[dev-dependencies.foo]
features = []

[dev-dependencies]
bar = { features = [], a = [] }

[dependencies]
bar = { features = [], a = [] }
",
        "\
[dependencies]
bar = { features = [], a = [] }
"
    );

    test!(
        many_lines,
        "\
[package]\n\n

[dev-dependencies.opencl]


[dev-dependencies]
",
        "\
[package]\n\n

"
    );

    test!(
        target_deps1,
        "\
[package]

[target.'cfg(unix)'.dev-dependencies]

[dependencies]
",
        "\
[package]

[dependencies]
"
    );

    test!(
        target_deps2,
        "\
[package]

[target.'cfg(unix)'.dev-dependencies]
foo = \"0.1\"

[target.'cfg(unix)'.dev-dependencies.bar]

[dev-dependencies]
foo = \"0.1\"

[target.'cfg(unix)'.dependencies]
foo = \"0.1\"
",
        "\
[package]

[target.'cfg(unix)'.dependencies]
foo = \"0.1\"
"
    );

    test!(
        target_deps3,
        "\
[package]

[target.'cfg(unix)'.dependencies]

[dev-dependencies]
",
        "\
[package]

[target.'cfg(unix)'.dependencies]

"
    );

    test!(
        target_deps4,
        "\
[package]

[target.'cfg(unix)'.dev-dependencies]
",
        "\
[package]

"
    );

    // NOTE: `a = [dev-dependencies]` is not valid TOML format.
    test!(
        not_table,
        "\
[package]
a = [dev-dependencies]
b = ['dev-dependencies']
c = [\"dev-dependencies\"]
# [dev-dependencies]

    [dev-dependencies]

    [dependencies]

    [target.'cfg(unix)'.dev-dependencies]
    foo = \"0.1\"


\t[dev-dependencies]
\tfoo = \"0.1\"
",
        "\
[package]
a = [dev-dependencies]
b = ['dev-dependencies']
c = [\"dev-dependencies\"]
# [dev-dependencies]

    [dependencies]

"
    );

    // NOTE: `foo = [[dev-dependencies]]` is not valid TOML format.
    test!(
        not_table_multi_line,
        "\
[package]
foo = [
    ['dev-dependencies'],
    [\"dev-dependencies\"]
]
",
        "\
[package]
foo = [
    ['dev-dependencies'],
    [\"dev-dependencies\"]
]
"
    );

    // Regression tests for bugs caught by fuzzing.
    #[test]
    fn fuzz() {
        let tests = &["'.'='''Mmm]\n\n[  dev-dependenciesh\t'''", "'.'='''M]\n[target.M'''"];
        for &test in tests {
            assert!(toml::from_str::<toml::Value>(test).is_ok());
            let result = remove_dev_deps(test);
            toml::from_str::<toml::Value>(&result).unwrap();
        }

        // TODO
        let fail_tests = &["'.'='''m\n[ dev-dependencies   ] '''"];
        for &test in fail_tests {
            assert!(toml::from_str::<toml::Value>(test).is_ok());
            let result = remove_dev_deps(test);
            toml::from_str::<toml::Value>(&result).unwrap_err();
        }
    }
}
