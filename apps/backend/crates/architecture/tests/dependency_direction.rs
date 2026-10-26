//! 層の依存方向の検査（docs/adr/0003-backend-layers.md）。
//!
//! `cargo metadata` でワークスペースの各クレートの依存（通常・dev・build のすべて）を読み、
//! 下の表にない辺があれば落とす。表にないクレートがワークスペースに増えても落とす
//! （層を決めずにクレートを足させない）。

use std::collections::BTreeSet;

use cargo_metadata::{Metadata, MetadataCommand};

/// 層のクレートと、依存してよいワークスペース内のクレート。
const LAYERS: &[(&str, &[&str])] = &[
    ("domain", &[]),
    ("application", &["domain"]),
    ("infra", &["domain", "application"]),
    ("interface", &["domain", "application"]),
];

/// 層の規約の対象外のクレート（検査そのもの）。ワークスペースの他のクレートに依存しない。
const TOOLING: &[&str] = &["architecture"];

/// domain が依存してはならない外部クレート（I/O・非同期ランタイム・フレームワーク）。
const DOMAIN_FORBIDDEN_EXTERNAL: &[&str] = &[
    "tokio",
    "async-std",
    "axum",
    "hyper",
    "tower",
    "reqwest",
    "sqlx",
    "diesel",
    "postgres",
    "tokio-postgres",
];

fn metadata() -> Metadata {
    MetadataCommand::new()
        .manifest_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
        .no_deps()
        .exec()
        .expect("cargo metadata を実行できない")
}

fn allowed(name: &str) -> Option<&'static [&'static str]> {
    LAYERS
        .iter()
        .find(|(layer, _)| *layer == name)
        .map(|(_, deps)| *deps)
        .or_else(|| TOOLING.contains(&name).then_some(&[][..]))
}

#[test]
fn workspace_crates_follow_layer_dependency_direction() {
    let metadata = metadata();
    let members: BTreeSet<&str> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    let mut violations = Vec::new();
    for package in metadata.workspace_packages() {
        let name = package.name.as_str();
        let Some(allowed) = allowed(name) else {
            violations.push(format!(
                "{name}: 層が決まっていない（tests/dependency_direction.rs の LAYERS に足す）"
            ));
            continue;
        };
        for dep in &package.dependencies {
            // 改名した依存（`package = "..."`）も実体の名前で見る。
            if members.contains(dep.name.as_str()) && !allowed.contains(&dep.name.as_str()) {
                violations.push(format!(
                    "{name} -> {} （{:?}）は許されない。許される依存: {allowed:?}",
                    dep.name, dep.kind
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "層の依存方向の違反（docs/adr/0003-backend-layers.md）:\n{}",
        violations.join("\n")
    );
}

#[test]
fn domain_has_no_io_dependencies() {
    let metadata = metadata();
    let domain = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == "domain")
        .expect("ワークスペースに domain がない");

    let violations: Vec<&str> = domain
        .dependencies
        .iter()
        .map(|d| d.name.as_str())
        .filter(|d| DOMAIN_FORBIDDEN_EXTERNAL.contains(d))
        .collect();

    assert!(
        violations.is_empty(),
        "domain は純粋に保つ（I/O なし）。依存してはならない: {violations:?}"
    );
}

#[test]
fn all_layers_exist_in_workspace() {
    let metadata = metadata();
    let members: BTreeSet<&str> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    for (layer, _) in LAYERS {
        assert!(members.contains(layer), "ワークスペースに {layer} がない");
    }
}
