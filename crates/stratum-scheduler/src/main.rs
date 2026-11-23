fn main() {
    // `env!` reads values Cargo sets at compile time from this crate's Cargo.toml.
    println!(
        "{}",
        stratum_core::banner(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    );
}
