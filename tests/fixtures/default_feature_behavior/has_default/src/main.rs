fn main() {
    #[cfg(feature = "default")]
    println!("has default feature!");
    #[cfg(not(feature = "default"))]
    println!("no default feature!");
}
