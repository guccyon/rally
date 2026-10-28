//! マイグレーションの規約の検査（docs/adr/0004-schema-expand-contract.md）。Postgres は要らない。
//!
//! - ファイル名は `<14 桁の版>_<段>_<説明>.sql`。段は expand / migrate / contract のどれか
//! - 前進だけ（sqlx の `.up.sql` / `.down.sql` は使わない）
//! - 破壊的な文（列・テーブルの削除、改名、型の変更、データの削除、NOT NULL の付与）は contract の段にだけ書ける。
//!   contract のマイグレーションを含む PR は人間承認（manifest の destructive_schema）
//! - 例外として、CHECK 制約を同じ名前で付け直す `DROP CONSTRAINT x, ADD CONSTRAINT x CHECK (...)`（1 つの ALTER TABLE）は
//!   expand に書ける（許す値を足す緩和。値を減らす付け直しは contract に分ける。ADR 0004 の 2）
//! - 例外として、[`RELAXABLE_UNIQUE_INDEXES`] に挙げた一意の索引の `DROP INDEX` は expand に書ける（一意性の緩和で、データは
//!   消えず、古い版のアプリも動く。挙げるときに ADR 0004 の 2 に理由を書く）

use std::fs;
use std::path::{Path, PathBuf};

const PHASES: &[&str] = &["expand", "migrate", "contract"];

/// expand で外してよい一意の索引（`<スキーマ>.<索引>`）。外しても古い版のアプリが動き、データも消えない緩和だけを、
/// ADR 0004 の 2 に理由を書いて挙げる。
///
/// - `scheduling.bookings_one_active_per_slot`: 1 枠 1 予約。スロットの定員（D40 / 変更シナリオ cs-004）で、同じ枠に定員まで
///   予約を持てるようにする。定員の判定は domain で、予約の作成はスロットの行のロックの中で数える
const RELAXABLE_UNIQUE_INDEXES: &[&str] = &["scheduling.bookings_one_active_per_slot"];

fn migration_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("migrations/ を読めない")
        .map(|entry| entry.expect("migrations/ を読めない").path())
        .collect();
    files.sort();
    files
}

/// ファイル名から段を取り出す。規約に合わなければ理由を返す。
fn phase_of(file_name: &str) -> Result<&str, String> {
    let Some(stem) = file_name.strip_suffix(".sql") else {
        return Err("拡張子が .sql でない".into());
    };
    if stem.ends_with(".up") || stem.ends_with(".down") {
        return Err("前進だけのマイグレーションにする（.up.sql / .down.sql は使わない）".into());
    }
    let mut parts = stem.splitn(3, '_');
    let (Some(version), Some(phase), Some(description)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return Err("<版>_<段>_<説明>.sql の形でない".into());
    };
    if version.len() != 14 || !version.bytes().all(|b| b.is_ascii_digit()) {
        return Err("版は 14 桁の数字（YYYYMMDDhhmmss）にする".into());
    }
    if !PHASES.contains(&phase) {
        return Err(format!("段は {PHASES:?} のどれか"));
    }
    if description.is_empty()
        || !description
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err("説明は英小文字スネークケースにする".into());
    }
    Ok(phase)
}

/// コメントと文字列リテラルを除き、英数字の語を大文字にして並べる。
fn tokens(sql: &str) -> Vec<String> {
    let mut cleaned = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '-' if chars.peek() == Some(&'-') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                cleaned.push(' ');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
                cleaned.push(' ');
            }
            '\'' => {
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                }
                cleaned.push(' ');
            }
            _ => cleaned.push(c),
        }
    }
    cleaned
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(str::to_ascii_uppercase)
        .collect()
}

/// `DROP CONSTRAINT x ADD CONSTRAINT x CHECK`（トークンの並び）で始まるか。
fn replaces_check_constraint(t: &[String]) -> bool {
    matches!(
        t,
        [drop, c1, name, add, c2, same, check, ..]
            if drop == "DROP" && c1 == "CONSTRAINT" && add == "ADD" && c2 == "CONSTRAINT"
                && check == "CHECK" && name == same
    )
}

/// `DROP INDEX [IF EXISTS] <スキーマ> <索引>`（トークンの並び）で、索引が [`RELAXABLE_UNIQUE_INDEXES`] にあるか。
/// `CASCADE` を付けたものは、索引に頼る制約も消すので対象外。
fn drops_relaxable_unique_index(t: &[String]) -> bool {
    let rest = match t {
        [drop, index, if_, exists, rest @ ..]
            if drop == "DROP" && index == "INDEX" && if_ == "IF" && exists == "EXISTS" =>
        {
            rest
        }
        [drop, index, rest @ ..] if drop == "DROP" && index == "INDEX" => rest,
        _ => return false,
    };
    match rest {
        [schema, name, after @ ..] => {
            after.first().is_none_or(|next| next != "CASCADE")
                && RELAXABLE_UNIQUE_INDEXES
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(&format!("{schema}.{name}")))
        }
        _ => false,
    }
}

/// contract の段にしか書けない破壊的な文を探す。
fn destructive_statements(sql: &str) -> Vec<String> {
    let t = tokens(sql);
    let at = |i: usize| t.get(i).map(String::as_str);
    let mut found = Vec::new();
    for i in 0..t.len() {
        let hit = match t[i].as_str() {
            // DROP NOT NULL / DROP DEFAULT は制約を緩めるだけなので expand に書ける
            // DROP CONSTRAINT x, ADD CONSTRAINT x CHECK は同じ名前の CHECK の付け直しなので expand に書ける
            // RELAXABLE_UNIQUE_INDEXES の DROP INDEX は一意性の緩和なので expand に書ける
            "DROP" => {
                !matches!(at(i + 1), Some("NOT" | "DEFAULT"))
                    && !replaces_check_constraint(&t[i..])
                    && !drops_relaxable_unique_index(&t[i..])
            }
            "RENAME" | "TRUNCATE" | "DELETE" => true,
            // ALTER [COLUMN] <列> TYPE / SET DATA TYPE（型の変更）
            "TYPE" => {
                (i >= 2 && at(i - 2) == Some("ALTER"))
                    || (i >= 3 && at(i - 3) == Some("ALTER") && at(i - 2) == Some("COLUMN"))
                    || (i >= 2 && at(i - 2) == Some("SET") && at(i - 1) == Some("DATA"))
            }
            // 既存の列への NOT NULL の付与（古い版の書き込みを壊す）
            "SET" => at(i + 1) == Some("NOT") && at(i + 2) == Some("NULL"),
            _ => false,
        };
        if hit {
            found.push(t[i..(i + 3).min(t.len())].join(" "));
        }
    }
    found
}

#[test]
fn migrations_follow_naming_and_phase_rules() {
    let files = migration_files();
    assert!(!files.is_empty(), "migrations/ にマイグレーションがない");

    let mut violations = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let phase = match phase_of(&name) {
            Ok(phase) => phase,
            Err(reason) => {
                violations.push(format!("{name}: {reason}"));
                continue;
            }
        };
        if phase == "contract" {
            continue;
        }
        let sql = fs::read_to_string(path).expect("マイグレーションを読めない");
        let destructive = destructive_statements(&sql);
        if !destructive.is_empty() {
            violations.push(format!(
                "{name}: 破壊的な文は contract の段に分ける（人間承認。ADR 0004）: {destructive:?}"
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn file_name_rules() {
    assert_eq!(phase_of("20261026000001_expand_identity.sql"), Ok("expand"));
    assert_eq!(
        phase_of("20261101000000_contract_drop_old_email.sql"),
        Ok("contract")
    );
    assert!(phase_of("20261026000001_identity.sql").is_err());
    assert!(phase_of("20261026000001_expand_identity.up.sql").is_err());
    assert!(phase_of("20261026000001_expand_identity.down.sql").is_err());
    assert!(phase_of("2026102601_expand_identity.sql").is_err());
    assert!(phase_of("20261026000001_expand_Identity.sql").is_err());
}

#[test]
fn detects_destructive_statements() {
    for sql in [
        "DROP TABLE identity.accounts;",
        "drop schema scheduling cascade;",
        "ALTER TABLE identity.accounts DROP COLUMN email;",
        "ALTER TABLE identity.accounts DROP email;",
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_email_check;",
        // 別の名前の制約を足しても付け直しではない
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_v2 CHECK (sport <> '');",
        // CHECK 以外（外部キー・一意）の付け直しは対象外
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_email_key, ADD CONSTRAINT accounts_email_key UNIQUE (email);",
        "DROP INDEX scheduling.slots_open_sport_starts_at_idx;",
        // 外してよい一意の索引でも、別のスキーマの同じ名前や CASCADE は対象外
        "DROP INDEX identity.bookings_one_active_per_slot;",
        "DROP INDEX bookings_one_active_per_slot;",
        "DROP INDEX scheduling.bookings_one_active_per_slot CASCADE;",
        "DROP INDEX scheduling.bookings_one_active_per_slot_student;",
        "ALTER TABLE identity.accounts RENAME COLUMN email TO mail;",
        "ALTER TABLE identity.accounts ALTER COLUMN display_name TYPE varchar(20);",
        "ALTER TABLE identity.accounts ALTER display_name TYPE varchar(20);",
        "ALTER TABLE identity.accounts ALTER COLUMN display_name SET DATA TYPE varchar(20);",
        "ALTER TABLE identity.accounts ALTER COLUMN sport SET NOT NULL;",
        "DELETE FROM identity.accounts;",
        "TRUNCATE scheduling.bookings;",
    ] {
        assert!(
            !destructive_statements(sql).is_empty(),
            "破壊的な文を見逃した: {sql}"
        );
    }
}

#[test]
fn allows_additive_statements() {
    for sql in [
        "CREATE TABLE identity.profiles (account_id uuid PRIMARY KEY, bio text NOT NULL);",
        "ALTER TABLE identity.accounts ADD COLUMN locale text;",
        "ALTER TABLE identity.accounts ALTER COLUMN sport DROP NOT NULL;",
        "ALTER TABLE identity.accounts ALTER COLUMN role DROP DEFAULT;",
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_check CHECK (sport IN ('tennis', 'swimming'));",
        "CREATE INDEX accounts_created_at_idx ON identity.accounts (created_at);",
        "DROP INDEX scheduling.bookings_one_active_per_slot;",
        "DROP INDEX IF EXISTS scheduling.bookings_one_active_per_slot;",
        "UPDATE identity.accounts SET locale = 'ja';",
        "-- DROP TABLE identity.accounts; はコメントなので数えない\nSELECT 1;",
        "/* RENAME */ COMMENT ON TABLE identity.accounts IS 'DROP TABLE と書いても文字列';",
    ] {
        assert_eq!(
            destructive_statements(sql),
            Vec::<String>::new(),
            "追加だけの文を破壊的とみなした: {sql}"
        );
    }
}
