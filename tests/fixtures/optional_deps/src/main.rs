fn main() {
    #[cfg(feature = "member2")]
    println!("member2");
    #[cfg(feature = "member3")]
    println!("member3");
    #[cfg(feature = "real")]
    println!("real");
    #[cfg(feature = "renamed")]
    println!("renamed");
}
