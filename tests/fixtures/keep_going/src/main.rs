#[cfg(feature = "a")]
compile_error!("`a` feature specified");
#[cfg(not(feature = "a"))]
compile_error!("`a` feature not specified");

fn main() {}
