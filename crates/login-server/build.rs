// `sqlx::migrate!` compiles the migrations in, but cargo does not see a new file there: rebuild when the folder changes, so a binary
// never misses a migration the database already has (it would refuse to start).
fn main() {
    println!("cargo:rerun-if-changed=../../db/migrations");
}
