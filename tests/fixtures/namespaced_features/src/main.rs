fn main() {
    // Actual features
    #[cfg(feature = "explicit")]
    println!("explicit");
    #[cfg(feature = "implicit")]
    println!("implicit");
    #[cfg(feature = "combo")]
    println!("combo");
    #[cfg(feature = "renamed")]
    println!("renamed");

    // Non-existent features
    #[cfg(feature = "member1")]
    println!("member1");
    #[cfg(feature = "member2")]
    println!("member2");
    #[cfg(feature = "member3")]
    println!("member3");
}
