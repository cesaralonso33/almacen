fn main() {
    // `sqlx::migrate!` embeds migrations at compile time. Track the directory so
    // incremental builds refresh every API binary when a migration is added or edited.
    println!("cargo:rerun-if-changed=migrations");
}
