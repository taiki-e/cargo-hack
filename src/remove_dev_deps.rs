use std::cmp;

pub(crate) fn remove_dev_deps(bytes: &str) -> String {
    const DEV_DEPS: &str = "dev-dependencies";
    const TARGET: &str = "target.";

    let mut bytes = bytes.to_string();
    let mut prev = 0;
    let mut next = bytes.find('[');

    'outer: while let Some(mut pos) = next {
        prev = bytes[prev..pos].rfind('\n').map_or(prev, |n| cmp::min(n + prev + 1, pos));

        // skip '# [...' and 'foo = [...'
        if bytes[prev..pos].trim().is_empty() {
            let slice = bytes[pos + 1..].trim_start();
            if slice.starts_with(DEV_DEPS) {
                let maybe_close = pos + DEV_DEPS.len();
                for (i, _) in bytes[maybe_close..].match_indices('[') {
                    let back = bytes[maybe_close..maybe_close + i]
                        .rfind('\n')
                        .map_or(0, |n| cmp::min(n + 1, i));

                    // skip '# [...' and 'foo = [...'
                    if bytes[maybe_close + back..maybe_close + i].trim().is_empty() {
                        bytes.drain(prev..maybe_close + back);
                        next = Some(prev + i - back);
                        continue 'outer;
                    }
                }

                bytes.drain(prev..);
                break;
            } else if slice.starts_with(TARGET) {
                let close = bytes[pos + TARGET.len()..].find(']').unwrap() + pos + TARGET.len();
                let mut split = bytes[pos..close].split('.');
                let _ = split.next(); // `target`
                let _ = split.next(); // `'cfg(...)'`
                if let Some(deps) = split.next() {
                    if deps.trim() == DEV_DEPS {
                        for (i, _) in bytes[close..].match_indices('[') {
                            let back = bytes[close..close + i]
                                .rfind('\n')
                                .map_or(0, |n| cmp::min(n + 1, i));

                            // skip '# [...' and 'foo = [...'
                            if bytes[close + back..close + i].trim().is_empty() {
                                bytes.drain(prev..close + back);
                                next = Some(prev + i - back);
                                continue 'outer;
                            }
                        }

                        bytes.drain(prev..);
                        break;
                    }
                }

                prev = pos;
                next = bytes[close..].find('[').map(|n| close + n);
                continue;
            }
        }

        prev = pos;
        // pos + 0 = '['
        // pos + 1 = part of table name (or '[')
        // pos + 2 = ']' (or part of table name)
        // pos + 3 = '\n' or eof (or part of table name or ']')
        // pos + 4 = start of next table or eof (or part of this table)
        pos += 4;
        next = bytes.get(pos..).and_then(|s| s.find('[')).map(|n| pos + n);
        continue;
    }

    bytes
}

#[cfg(test)]
mod tests {

    macro_rules! test {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let input = super::remove_dev_deps($input);
                assert_eq!(&$expected[..], input);
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
        not_table,
        "\
[package]
foo = [dev-dependencies]
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
foo = [dev-dependencies]
# [dev-dependencies]

    [dependencies]

"
    );
}
