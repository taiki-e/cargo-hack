fn main() {
    println!("hello!");
    #[cfg(feature = "default")]
    println!("default");
    #[cfg(feature = "a")]
    println!("a");
    #[cfg(feature = "b")]
    println!("b");
    #[cfg(feature = "c")]
    println!("c");
}
