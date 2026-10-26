# apps/backend

Rally のバックエンド（Rust / axum / PostgreSQL）の Cargo ワークスペース。層の規約と依存方向は [ADR 0003](../../docs/adr/0003-backend-layers.md)。

## 構成

| パス | 層 | 依存してよいクレート |
|---|---|---|
| `crates/domain/` | ドメイン。純粋（I/O なし）。`identity`（アカウント・役割・スポーツと不変条件） | なし |
| `crates/application/` | ユースケースとポート（`identity::RegisterAccount`・`AccountRepository`・`AnalyticsSink`・`Clock`）。ポートのインメモリ実装（`in_memory`） | domain |
| `crates/infra/` | Postgres（`migrations/`）・モックの外部連携 | domain・application |
| `crates/interface/` | axum の HTTP（`GET /healthz`、`POST /accounts`） | domain・application（infra は不可） |
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
curl -i -X POST http://127.0.0.1:8080/accounts -H 'content-type: application/json' \
  -d '{"email":"taro@example.com","display_name":"たろう","sport":"tennis"}'   # 201
```

ポート（アカウントの保存先・行動トラッキング）は当面 `application::in_memory` のインメモリ実装で組み立てるので、登録したアカウントはプロセスを止めると消える。Postgres の実装は infra に置き、組み立て用のクレートを足すときに差し替える（ADR 0003 の決定 5）。

## identity の不変条件

`domain::identity` がモデルの生成時に検証し、破れば `DomainError` を返す。

- 役割（`student` / `coach` / `headquarters`。DB の `identity.accounts.role` と同じ値）とスポーツの組み合わせ: 生徒はちょうど 1 つ、コーチは 1 つ以上（提供スポーツの集合）、本部はなし
- スポーツは Blueprint の 5 種類（tennis / golf / baseball / soccer / basketball）のどれか。student-app の `POST /accounts` はさらにゴルフを 422 で断る（契約の `Sport` がゴルフを含まない）
- 表示名は空（空白だけ）でなく 50 文字まで（契約の `maxLength`）。メールアドレスは `@` の前後が空でない最小の形で、一意性は大文字小文字を区別しない

`POST /accounts` の統合テスト（`crates/interface/tests/accounts.rs`）は、レスポンスを `specs/contracts/api/student-app/openapi.yaml` のスキーマで検査する。生徒の登録で行動イベント `account.registered` が 1 回発火することもここで確かめる。

## CI で走らせるコマンド

`apps/backend/` で次の 3 つを順に走らせ、どれかが落ちたらマージしない（L1）。依存方向の検査は `cargo test` に含まれる（`crates/architecture/tests/dependency_direction.rs`）。

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
