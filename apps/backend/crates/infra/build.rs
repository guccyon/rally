// migrations/ にファイルを足したら `sqlx::migrate!()` を埋め込み直す。
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
