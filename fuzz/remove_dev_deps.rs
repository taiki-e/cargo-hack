#![no_main]

#[path = "../src/remove_dev_deps.rs"]
mod remove_dev_deps;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|string: &str| {
    if toml::from_str::<toml::Value>(string).is_ok() {
        let result = remove_dev_deps::remove_dev_deps(string);
        toml::from_str::<toml::Value>(&result).unwrap();
    }
});
