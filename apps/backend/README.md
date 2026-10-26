# apps/backend

Rally のバックエンド（Rust / axum / PostgreSQL）の Cargo ワークスペース。層の規約と依存方向は [ADR 0003](../../docs/adr/0003-backend-layers.md)。

## 構成

| パス | 層 | 依存してよいクレート |
|---|---|---|
| `crates/domain/` | ドメイン。純粋（I/O なし） | なし |
| `crates/application/` | ユースケースとポート | domain |
| `crates/infra/` | Postgres（`migrations/`）・モックの外部連携 | domain・application |
| `crates/interface/` | axum の HTTP（`GET /healthz`） | domain・application（infra は不可） |
| `crates/architecture/` | 依存方向の検査（テストだけ） | なし |

ツールチェーンは `rust-toolchain.toml`（stable、rustfmt・clippy 付き）。

## データベース

ローカルの Postgres は `compose.yaml`（Docker Compose）で起動する。スキーマは `crates/infra/migrations/` の sqlx のマイグレーションで、変更は expand → 移行 → contract の 3 段に分ける（[ADR 0004](../../docs/adr/0004-schema-expand-contract.md)）。

```sh
docker compose up -d --wait                       # 127.0.0.1:5432 に Postgres 16
export DATABASE_URL=postgres://rally@127.0.0.1:5432/rally
cargo install sqlx-cli --no-default-features --features postgres,rustls   # 初回だけ
cargo sqlx migrate run --source crates/infra/migrations
cargo sqlx migrate info --source crates/infra/migrations
```

マイグレーションのファイル名は `<14 桁の版>_<段>_<説明>.sql`（段は `expand` / `migrate` / `contract`）。規約は `crates/infra/tests/migration_conventions.rs` が `cargo test` で検査する。

スキーマの検証（`crates/infra/tests/schema.rs`）は Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して次で走らせる（テストごとに新しい DB を作ってマイグレーションを当てる）。

```sh
cargo test -p infra -- --include-ignored
```

## 起動

```sh
cargo run -p interface            # 127.0.0.1:8080 で待ち受ける
BIND_ADDR=0.0.0.0:3000 cargo run -p interface
curl -i http://127.0.0.1:8080/healthz   # 200 ok
```

## CI で走らせるコマンド

`apps/backend/` で次の 3 つを順に走らせ、どれかが落ちたらマージしない（L1）。依存方向の検査は `cargo test` に含まれる（`crates/architecture/tests/dependency_direction.rs`）。

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
