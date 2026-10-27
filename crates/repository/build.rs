//! Re-embeds the API migrations in this crate's test migrator when one is
//! added, removed or changed: `sqlx::migrate!` does not track the directory.

fn main() {
    println!("cargo:rerun-if-changed=../../apps/api/migrations");
}
