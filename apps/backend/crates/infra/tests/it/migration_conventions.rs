//! マイグレーションの規約の検査（docs/adr/0004-schema-expand-contract.md）。Postgres は要らない。
//!
//! - ファイル名は `<14 桁の版>_<段>_<説明>.sql`。段は expand / migrate / contract のどれか
//! - 前進だけ（sqlx の `.up.sql` / `.down.sql` は使わない）
//! - 破壊的な文（列・テーブルの削除、改名、型の変更、データの削除、NOT NULL の付与）は contract の段にだけ書ける。
//!   contract のマイグレーションを含む PR は人間承認（manifest の destructive_schema）
//! - 緩和（データは消えず、古い版のアプリもそのまま動く変更）は expand / 移行に書ける。形は次の 3 つ（ADR 0004 の 2）
//!   - `DROP NOT NULL` / `DROP DEFAULT`
//!   - CHECK 制約の同じ名前の付け直し（`DROP CONSTRAINT x, ADD CONSTRAINT x CHECK (...)` を 1 つの ALTER TABLE に）。
//!     前の定義と新しい定義がどちらも `<列> IN (<値>, ...)` なら値の増減を比べ、足すだけなら通し、減らすなら止める。
//!     比べられない形は、文に `-- relax: 理由` のコメントがあるときだけ通す
//!   - 一意・外部キーの制約の `ALTER TABLE ... DROP CONSTRAINT` と、一意の索引の `DROP INDEX`。文に `-- relax: 理由` の
//!     コメントがあるときだけ通す。`CASCADE` を付けたものは、頼っている他のものも消すので止める
//! - 制約・索引の種類と CHECK の定義は、前のマイグレーション（同じファイルの前の文を含む）から読み取る（[`Catalog`]）

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const PHASES: &[&str] = &["expand", "migrate", "contract"];

/// 緩和の理由を書く行コメントの書き出し（`-- relax: 理由`）。
const RELAX_MARKER: &str = "relax:";

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

/// `;` で区切った 1 つの文。
#[derive(Default)]
struct Statement {
    /// 英数字の語（大文字）・文字列リテラル（`'...'` のまま）・記号 `(` `)` `,`。コメントと `.` などの記号は除く
    tokens: Vec<String>,
    /// 文の前か中に、理由の書かれた `-- relax: 理由` があったか
    relax: bool,
}

/// SQL を文に分け、文ごとにトークンを並べる。
fn statements(sql: &str) -> Vec<Statement> {
    let mut statements = Vec::new();
    let mut current = Statement::default();
    let mut word = String::new();
    let mut chars = sql.chars().peekable();
    let flush = |word: &mut String, current: &mut Statement| {
        if !word.is_empty() {
            current.tokens.push(word.to_ascii_uppercase());
            word.clear();
        }
    };
    while let Some(c) = chars.next() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        flush(&mut word, &mut current);
        match c {
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                let comment: String = chars.by_ref().take_while(|&c| c != '\n').collect();
                if comment
                    .trim()
                    .strip_prefix(RELAX_MARKER)
                    .is_some_and(|reason| !reason.trim().is_empty())
                {
                    current.relax = true;
                }
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
            }
            '\'' => {
                let mut literal = String::from('\'');
                while let Some(c) = chars.next() {
                    literal.push(c);
                    if c == '\'' {
                        // '' は引用符そのもの
                        if chars.peek() == Some(&'\'') {
                            literal.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                }
                current.tokens.push(literal);
            }
            '(' | ')' | ',' => current.tokens.push(c.to_string()),
            ';' => {
                if !current.tokens.is_empty() {
                    statements.push(std::mem::take(&mut current));
                }
                current = Statement::default();
            }
            _ => {}
        }
    }
    flush(&mut word, &mut current);
    if !current.tokens.is_empty() {
        statements.push(current);
    }
    statements
}

fn is_word(token: &str) -> bool {
    token
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// `t[0]` が `(` なら、対応する `)` までの中身を返す。
fn group(t: &[String]) -> Option<&[String]> {
    if t.first()? != "(" {
        return None;
    }
    let mut depth = 0;
    for (i, token) in t.iter().enumerate() {
        match token.as_str() {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Some(&t[1..i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// 括弧の外の `,` で分ける。
fn split_top_level(t: &[String]) -> Vec<&[String]> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, token) in t.iter().enumerate() {
        match token.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            "," if depth == 0 => {
                parts.push(&t[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&t[start..]);
    parts
}

/// 先頭の `IF EXISTS` / `IF NOT EXISTS` を飛ばす。
fn skip_if_exists(t: &[String]) -> &[String] {
    match t {
        [if_, not, exists, rest @ ..] if if_ == "IF" && not == "NOT" && exists == "EXISTS" => rest,
        [if_, exists, rest @ ..] if if_ == "IF" && exists == "EXISTS" => rest,
        _ => t,
    }
}

/// `CREATE TABLE` / `ALTER TABLE` の文の表（`<スキーマ>.<テーブル>`）と、その後のトークン。
fn table_of(t: &[String]) -> Option<(String, &[String])> {
    let [verb, table, rest @ ..] = t else {
        return None;
    };
    if !(verb == "CREATE" || verb == "ALTER") || table != "TABLE" {
        return None;
    }
    let rest = skip_if_exists(rest);
    let rest = match rest {
        [only, rest @ ..] if only == "ONLY" => rest,
        _ => rest,
    };
    match rest {
        [schema, name, rest @ ..] if is_word(schema) && is_word(name) => {
            Some((format!("{schema}.{name}"), rest))
        }
        _ => None,
    }
}

/// 前のマイグレーションで作られた制約と索引のうち、緩和の判定に要るもの。名前は大文字。
#[derive(Default)]
struct Catalog {
    /// 一意・外部キーの制約（`<スキーマ>.<テーブル>.<制約>`）
    keys: HashSet<String>,
    /// 一意の索引（`<スキーマ>.<索引>`）
    unique_indexes: HashSet<String>,
    /// CHECK 制約（`<スキーマ>.<テーブル>.<制約>`）の `CHECK` の後の括弧の中
    checks: HashMap<String, Vec<String>>,
}

impl Catalog {
    /// 文が作る・消す制約と索引を覚える。
    fn apply(&mut self, statement: &Statement) {
        let t = statement.tokens.as_slice();
        if let Some((table, rest)) = table_of(t) {
            if t[0] == "CREATE" {
                for element in group(rest).map(split_top_level).unwrap_or_default() {
                    self.define(&table, element);
                }
                return;
            }
            for action in split_top_level(rest) {
                match action {
                    [add, column, rest @ ..] if add == "ADD" && column == "COLUMN" => {
                        self.define(&table, skip_if_exists(rest))
                    }
                    [add, rest @ ..] if add == "ADD" => self.define(&table, rest),
                    [drop, constraint, rest @ ..]
                        if drop == "DROP" && constraint == "CONSTRAINT" =>
                    {
                        if let [name, ..] = skip_if_exists(rest) {
                            let key = format!("{table}.{name}");
                            self.keys.remove(&key);
                            self.checks.remove(&key);
                        }
                    }
                    _ => {}
                }
            }
            return;
        }
        match t {
            [create, unique, index, rest @ ..]
                if create == "CREATE" && unique == "UNIQUE" && index == "INDEX" =>
            {
                if let [name, on, rest @ ..] = skip_if_exists(rest) {
                    let rest = match rest {
                        [only, rest @ ..] if only == "ONLY" => rest,
                        _ => rest,
                    };
                    if let [schema, ..] = rest
                        && on == "ON"
                    {
                        self.unique_indexes.insert(format!("{schema}.{name}"));
                    }
                }
            }
            [drop, index, rest @ ..] if drop == "DROP" && index == "INDEX" => {
                for target in split_top_level(skip_if_exists(rest)) {
                    if let [schema, name, ..] = target {
                        self.unique_indexes.remove(&format!("{schema}.{name}"));
                    }
                }
            }
            _ => {}
        }
    }

    /// 表の定義の 1 つの要素（列の定義か表の制約）を覚える。
    fn define(&mut self, table: &str, element: &[String]) {
        match element {
            [constraint, name, kind, rest @ ..] if constraint == "CONSTRAINT" => {
                self.constraint(table, name, kind, rest)
            }
            // 名前のない表の制約は、Postgres の付ける名前を推し量らない（外すときに比べられない）
            [first, ..]
                if ["PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "EXCLUDE", "LIKE"]
                    .contains(&first.as_str()) => {}
            [column, rest @ ..] => {
                // 名前のない列の制約は Postgres の既定の名前（<テーブル>_<列>_check / _key / _fkey）で持つ
                let table_name = table.rsplit('.').next().unwrap_or(table);
                let mut i = 0;
                while i < rest.len() {
                    let (name, kind_at) = if rest[i] == "CONSTRAINT" {
                        (rest.get(i + 1).cloned(), i + 2)
                    } else {
                        (None, i)
                    };
                    let Some(kind) = rest.get(kind_at) else {
                        break;
                    };
                    let suffix = match kind.as_str() {
                        "CHECK" => Some("CHECK"),
                        "UNIQUE" => Some("KEY"),
                        "REFERENCES" => Some("FKEY"),
                        _ => None,
                    };
                    if let Some(suffix) = suffix {
                        let name =
                            name.unwrap_or_else(|| format!("{table_name}_{column}_{suffix}"));
                        self.constraint(table, &name, kind, &rest[kind_at + 1..]);
                    }
                    i = kind_at + 1;
                }
            }
            [] => {}
        }
    }

    fn constraint(&mut self, table: &str, name: &str, kind: &str, rest: &[String]) {
        let key = format!("{table}.{name}");
        match kind {
            "UNIQUE" | "FOREIGN" | "REFERENCES" => {
                self.keys.insert(key);
            }
            "CHECK" => {
                if let Some(expression) = group(rest) {
                    self.checks.insert(key, expression.to_vec());
                }
            }
            _ => {}
        }
    }
}

/// CHECK 制約の付け直しで、許す値がどう変わるか。
#[derive(Debug, PartialEq)]
enum CheckChange {
    /// 値を足すだけ（同じを含む）
    Widens,
    /// 値を減らす
    Narrows,
    /// 前の定義がないか、`<列> IN (<値>, ...)` の形でなく比べられない
    Incomparable,
}

/// `<列> IN ('<値>', ...)` の形なら、列と値の集合を返す。
fn in_list(expression: &[String]) -> Option<(&str, HashSet<&str>)> {
    let [column, in_, rest @ ..] = expression else {
        return None;
    };
    let values = group(rest)?;
    if in_ != "IN" || values.len() + 2 != rest.len() {
        return None;
    }
    split_top_level(values)
        .into_iter()
        .map(|value| match value {
            [literal] if literal.starts_with('\'') => Some(literal.as_str()),
            _ => None,
        })
        .collect::<Option<HashSet<_>>>()
        .map(|values| (column.as_str(), values))
}

fn compare_check(before: Option<&[String]>, after: &[String]) -> CheckChange {
    let Some(before) = before else {
        return CheckChange::Incomparable;
    };
    if before == after {
        return CheckChange::Widens;
    }
    match (in_list(before), in_list(after)) {
        (Some((column, old)), Some((same, new))) if column == same => {
            if old.is_subset(&new) {
                CheckChange::Widens
            } else {
                CheckChange::Narrows
            }
        }
        _ => CheckChange::Incomparable,
    }
}

/// `t[i]` の `DROP` が緩和なら `Ok`、破壊的なら理由を返す。`table` は文の `ALTER TABLE` の表。
fn relaxation(
    t: &[String],
    i: usize,
    table: Option<&str>,
    relax: bool,
    catalog: &Catalog,
) -> Result<(), &'static str> {
    const NEEDS_REASON: &str = "緩和なら -- relax: に理由を書く";
    match &t[i + 1..] {
        [next, ..] if next == "NOT" || next == "DEFAULT" => Ok(()),
        [constraint, rest @ ..] if constraint == "CONSTRAINT" => {
            let Some(table) = table else {
                return Err("ALTER TABLE の外の DROP CONSTRAINT");
            };
            let [name, after @ ..] = skip_if_exists(rest) else {
                return Err("制約の名前がない");
            };
            let key = format!("{table}.{name}");
            match after {
                // 同じ名前の CHECK の付け直し
                [comma, add, constraint, same, check, expression @ ..]
                    if comma == ","
                        && add == "ADD"
                        && constraint == "CONSTRAINT"
                        && same == name
                        && check == "CHECK" =>
                {
                    let after = group(expression).ok_or("CHECK の式がない")?;
                    match compare_check(catalog.checks.get(&key).map(Vec::as_slice), after) {
                        CheckChange::Widens => Ok(()),
                        CheckChange::Narrows => Err("許す値を減らす CHECK の付け直し"),
                        CheckChange::Incomparable if relax => Ok(()),
                        CheckChange::Incomparable => Err(
                            "値の増減を比べられない CHECK の付け直し。緩和なら -- relax: に理由を書く",
                        ),
                    }
                }
                [next, ..] if next != "," && next != "RESTRICT" => {
                    Err("CASCADE などの付いた制約の削除")
                }
                _ if !catalog.keys.contains(&key) => {
                    Err("一意・外部キーでない（前のマイグレーションにない）制約の削除")
                }
                _ if !relax => Err(NEEDS_REASON),
                _ => Ok(()),
            }
        }
        [index, rest @ ..] if index == "INDEX" => match skip_if_exists(rest) {
            [schema, name] if catalog.unique_indexes.contains(&format!("{schema}.{name}")) => {
                if relax {
                    Ok(())
                } else {
                    Err(NEEDS_REASON)
                }
            }
            [_, _] => Err("一意でない（前のマイグレーションにない）索引の削除"),
            _ => Err("スキーマのない・CASCADE の付いた・複数の索引の削除"),
        },
        _ => Err("列・テーブル・スキーマなどの削除"),
    }
}

/// 1 つの文から、contract の段にしか書けない破壊的な文を探す。
fn destructive_in_statement(statement: &Statement, catalog: &Catalog) -> Vec<String> {
    let t = statement.tokens.as_slice();
    let at = |i: usize| t.get(i).map(String::as_str);
    let table = table_of(t)
        .filter(|_| t[0] == "ALTER")
        .map(|(table, _)| table);
    let mut found = Vec::new();
    for i in 0..t.len() {
        let hit = match t[i].as_str() {
            "DROP" => relaxation(t, i, table.as_deref(), statement.relax, catalog).err(),
            "RENAME" | "TRUNCATE" | "DELETE" => Some("改名・データの削除"),
            // ALTER [COLUMN] <列> TYPE / SET DATA TYPE（型の変更）
            "TYPE"
                if (i >= 2 && at(i - 2) == Some("ALTER"))
                    || (i >= 3 && at(i - 3) == Some("ALTER") && at(i - 2) == Some("COLUMN"))
                    || (i >= 2 && at(i - 2) == Some("SET") && at(i - 1) == Some("DATA")) =>
            {
                Some("型の変更")
            }
            // 既存の列への NOT NULL の付与（古い版の書き込みを壊す）
            "SET" if at(i + 1) == Some("NOT") && at(i + 2) == Some("NULL") => {
                Some("NOT NULL の付与")
            }
            _ => None,
        };
        if let Some(reason) = hit {
            found.push(format!(
                "{}（{reason}）",
                t[i..(i + 3).min(t.len())].join(" ")
            ));
        }
    }
    found
}

/// contract の段にしか書けない破壊的な文を探す。`catalog` は前のマイグレーションまでを覚えたもので、この SQL の分も覚える。
fn destructive_statements(sql: &str, catalog: &mut Catalog) -> Vec<String> {
    let mut found = Vec::new();
    for statement in statements(sql) {
        found.extend(destructive_in_statement(&statement, catalog));
        catalog.apply(&statement);
    }
    found
}

#[test]
fn migrations_follow_naming_and_phase_rules() {
    let files = migration_files();
    assert!(!files.is_empty(), "migrations/ にマイグレーションがない");

    let mut catalog = Catalog::default();
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
        let sql = fs::read_to_string(path).expect("マイグレーションを読めない");
        let destructive = destructive_statements(&sql, &mut catalog);
        if phase != "contract" && !destructive.is_empty() {
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

/// 下の試験の前のマイグレーション（制約と索引の種類・CHECK の定義を読み取らせる）。
const EARLIER_MIGRATIONS: &str = "
CREATE TABLE identity.accounts (
    account_id uuid PRIMARY KEY,
    email      text NOT NULL,
    role       text NOT NULL CHECK (role IN ('student', 'coach')),
    sport      text CHECK (sport IN ('tennis', 'golf')),
    CONSTRAINT accounts_account_id_role_key UNIQUE (account_id, role),
    CONSTRAINT accounts_sport_only_for_students CHECK ((role = 'student') = (sport IS NOT NULL))
);
CREATE UNIQUE INDEX accounts_email_key ON identity.accounts (lower(email));
CREATE INDEX accounts_created_at_idx ON identity.accounts (created_at);
CREATE TABLE scheduling.slots (
    slot_id  uuid PRIMARY KEY,
    coach_id uuid NOT NULL REFERENCES identity.accounts (account_id),
    capacity smallint NOT NULL CHECK (capacity BETWEEN 1 AND 6)
);
ALTER TABLE scheduling.slots
    ADD COLUMN status text NOT NULL DEFAULT 'open'
        CONSTRAINT slots_status_check CHECK (status IN ('open', 'booked')),
    ADD CONSTRAINT slots_coach_role_fkey FOREIGN KEY (coach_id, coach_role)
        REFERENCES identity.accounts (account_id, role);
";

fn destructive_after_earlier(sql: &str) -> Vec<String> {
    let mut catalog = Catalog::default();
    assert!(destructive_statements(EARLIER_MIGRATIONS, &mut catalog).is_empty());
    destructive_statements(sql, &mut catalog)
}

#[test]
fn detects_destructive_statements() {
    for sql in [
        "DROP TABLE identity.accounts;",
        "drop schema scheduling cascade;",
        "ALTER TABLE identity.accounts DROP COLUMN email;",
        "ALTER TABLE identity.accounts DROP email;",
        // 理由があっても、列・表の削除は緩和ではない
        "-- relax: 使わなくなった\nALTER TABLE identity.accounts DROP COLUMN email;",
        "-- relax: 使わなくなった\nDROP TABLE scheduling.slots;",
        "ALTER TABLE identity.accounts RENAME COLUMN email TO mail;",
        "ALTER TABLE identity.accounts ALTER COLUMN display_name TYPE varchar(20);",
        "ALTER TABLE identity.accounts ALTER display_name TYPE varchar(20);",
        "ALTER TABLE identity.accounts ALTER COLUMN display_name SET DATA TYPE varchar(20);",
        "ALTER TABLE identity.accounts ALTER COLUMN sport SET NOT NULL;",
        "DELETE FROM identity.accounts;",
        "TRUNCATE scheduling.bookings;",
    ] {
        assert!(
            !destructive_after_earlier(sql).is_empty(),
            "破壊的な文を見逃した: {sql}"
        );
    }
}

#[test]
fn stops_check_replacements_that_are_not_relaxations() {
    for sql in [
        // 許す値を減らす付け直しは、理由があっても止める
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_check CHECK (sport IN ('tennis'));",
        "-- relax: ゴルフをやめる\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_check CHECK (sport IN ('tennis', 'baseball'));",
        "ALTER TABLE scheduling.slots DROP CONSTRAINT slots_status_check, ADD CONSTRAINT slots_status_check CHECK (status IN ('open'));",
        // 比べられない付け直し（IN の形でない・列が違う・前の定義がない）は理由がなければ止める
        "ALTER TABLE scheduling.slots DROP CONSTRAINT slots_capacity_check, ADD CONSTRAINT slots_capacity_check CHECK (capacity BETWEEN 1 AND 8);",
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_check CHECK (role IN ('tennis', 'golf', 'swimming'));",
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_locale_check, ADD CONSTRAINT accounts_locale_check CHECK (locale IN ('ja'));",
        // 別の名前の制約を足しても付け直しではない（CHECK の削除）
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_v2 CHECK (sport <> '');",
        "-- relax: 新しい名前に\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_v2 CHECK (sport <> '');",
        // CHECK の削除は、理由があっても緩和の形ではない
        "-- relax: 形の制約を外す\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_only_for_students;",
        // CASCADE の付いた付け直しは形に合わない
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check CASCADE, ADD CONSTRAINT accounts_sport_check CHECK (sport IN ('tennis', 'golf', 'swimming'));",
    ] {
        assert!(
            !destructive_after_earlier(sql).is_empty(),
            "緩和でない CHECK の付け直しを見逃した: {sql}"
        );
    }
}

#[test]
fn stops_key_and_index_drops_without_a_relaxation() {
    for sql in [
        // 一意・外部キーの削除でも、理由がなければ止める
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_account_id_role_key;",
        "DROP INDEX identity.accounts_email_key;",
        // 理由の欄が空・別の文の理由は数えない
        "-- relax:\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_account_id_role_key;",
        "-- relax: 前の文の理由\nSELECT 1;\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_account_id_role_key;",
        "-- 理由: relax: と書き出していない\nDROP INDEX identity.accounts_email_key;",
        // CASCADE は頼っている他の制約も消すので、理由があっても止める
        "-- relax: 一意を外す\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_account_id_role_key CASCADE;",
        "-- relax: 一意を外す\nDROP INDEX identity.accounts_email_key CASCADE;",
        // 一意・外部キーでない・前のマイグレーションにない制約と索引は、理由があっても止める
        "-- relax: 主キーを外す\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_pkey;",
        "-- relax: 索引を外す\nDROP INDEX identity.accounts_created_at_idx;",
        "-- relax: ない制約\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_locale_key;",
        // 別の表・別のスキーマの同じ名前、スキーマのない名前、ALTER TABLE の外、複数の索引は止める
        "-- relax: 一意を外す\nALTER TABLE scheduling.slots DROP CONSTRAINT accounts_account_id_role_key;",
        "-- relax: 一意を外す\nDROP INDEX scheduling.accounts_email_key;",
        "-- relax: 一意を外す\nDROP INDEX accounts_email_key;",
        "-- relax: 一意を外す\nDROP CONSTRAINT accounts_account_id_role_key;",
        "-- relax: 一意を外す\nDROP INDEX identity.accounts_email_key, identity.accounts_created_at_idx;",
        // 同じ文で付け直しと一緒に外しても、理由がなければ止める
        "ALTER TABLE scheduling.slots DROP CONSTRAINT slots_status_check, ADD CONSTRAINT slots_status_check CHECK (status IN ('open', 'booked', 'closed')), DROP CONSTRAINT slots_coach_id_fkey;",
    ] {
        assert!(
            !destructive_after_earlier(sql).is_empty(),
            "理由のない・緩和でない削除を見逃した: {sql}"
        );
    }
}

#[test]
fn allows_additive_statements_and_relaxations() {
    for sql in [
        "CREATE TABLE identity.profiles (account_id uuid PRIMARY KEY, bio text NOT NULL);",
        "ALTER TABLE identity.accounts ADD COLUMN locale text;",
        "ALTER TABLE identity.accounts ALTER COLUMN sport DROP NOT NULL;",
        "ALTER TABLE identity.accounts ALTER COLUMN role DROP DEFAULT;",
        "CREATE INDEX accounts_created_at_idx ON identity.accounts (created_at);",
        "UPDATE identity.accounts SET locale = 'ja';",
        "-- DROP TABLE identity.accounts; はコメントなので数えない\nSELECT 1;",
        "/* RENAME */ COMMENT ON TABLE identity.accounts IS 'DROP TABLE と書いても文字列';",
        "COMMENT ON TABLE identity.accounts IS 'it''s; DROP TABLE identity.accounts';",
        // 値を足すだけの CHECK の付け直し（名前のない列の制約・名前のある制約。並びが変わってもよい）は理由なしで通す
        "ALTER TABLE identity.accounts DROP CONSTRAINT accounts_sport_check, ADD CONSTRAINT accounts_sport_check CHECK (sport IN ('tennis', 'golf', 'swimming'));",
        "ALTER TABLE identity.accounts DROP CONSTRAINT IF EXISTS accounts_sport_check, ADD CONSTRAINT accounts_sport_check CHECK (sport IN ('golf', 'swimming', 'tennis'));",
        "ALTER TABLE scheduling.slots ALTER COLUMN status DROP DEFAULT, DROP CONSTRAINT slots_status_check, ADD CONSTRAINT slots_status_check CHECK (status IN ('open', 'booked', 'closed'));",
        // 比べられない付け直しは理由があれば通す
        "-- relax: 定員を 8 人まで広げる\nALTER TABLE scheduling.slots DROP CONSTRAINT slots_capacity_check, ADD CONSTRAINT slots_capacity_check CHECK (capacity BETWEEN 1 AND 8);",
        // 一意・外部キーの制約（名前のある・名前のない列の制約）と一意の索引は、理由があれば外せる
        "-- relax: 一意性を緩める\nALTER TABLE identity.accounts DROP CONSTRAINT accounts_account_id_role_key;",
        "-- relax: 一意性を緩める\nALTER TABLE identity.accounts DROP CONSTRAINT IF EXISTS accounts_account_id_role_key RESTRICT;",
        "-- relax: 参照を外す\nALTER TABLE scheduling.slots DROP CONSTRAINT slots_coach_id_fkey;",
        "ALTER TABLE scheduling.slots\n    -- relax: 参照を外す（文の中の理由も数える）\n    DROP CONSTRAINT slots_coach_role_fkey;",
        "-- relax: 一意性を緩める\nDROP INDEX identity.accounts_email_key;",
        "-- relax: 一意性を緩める\nDROP INDEX IF EXISTS identity.accounts_email_key;",
        // 同じファイルで足した一意の索引も外せる
        "CREATE UNIQUE INDEX slots_status_key ON scheduling.slots (status);\n-- relax: 一意性を緩める\nDROP INDEX scheduling.slots_status_key;",
    ] {
        assert_eq!(
            destructive_after_earlier(sql),
            Vec::<String>::new(),
            "追加か緩和だけの文を破壊的とみなした: {sql}"
        );
    }
}

#[test]
fn compares_check_values() {
    let tokens = |sql: &str| statements(sql).remove(0).tokens;
    let tennis_golf = tokens("sport IN ('tennis', 'golf')");
    assert_eq!(
        compare_check(
            Some(tennis_golf.as_slice()),
            &tokens("sport IN ('golf', 'tennis', 'swimming')")
        ),
        CheckChange::Widens
    );
    assert_eq!(
        compare_check(Some(tennis_golf.as_slice()), &tokens("sport IN ('tennis')")),
        CheckChange::Narrows
    );
    assert_eq!(
        compare_check(
            Some(tennis_golf.as_slice()),
            &tokens("sport IN ('tennis', 'golf') OR sport IS NULL")
        ),
        CheckChange::Incomparable
    );
    assert_eq!(
        compare_check(None, &tokens("sport IN ('tennis')")),
        CheckChange::Incomparable
    );
}
