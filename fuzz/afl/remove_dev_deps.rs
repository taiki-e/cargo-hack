#[path = "../../src/remove_dev_deps.rs"]
mod remove_dev_deps;

use std::str;

use afl::fuzz;

fn main() {
    fuzz!(|bytes: &[u8]| {
        if let Ok(string) = str::from_utf8(bytes) {
            if toml::from_str::<toml::Value>(string).is_ok() {
                let result = remove_dev_deps::remove_dev_deps(string);
                toml::from_str::<toml::Value>(&result).unwrap();
            }
        }
    });
}
