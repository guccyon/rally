# ADR 0003: バックエンドの層と依存方向の検査

- 状態: 提案（2026-10-26、T01 / Issue #19）
- 関係: `docs/plans/p1-walking-skeleton.md`（段 1 の T01、L1 の依存方向テスト）、`apps/backend/`

## 決定

1. バックエンド（Rust / axum / PostgreSQL）は `apps/backend/` の Cargo ワークスペースに置き、層ごとに 1 クレートに分ける

   | クレート | 役割 | 依存してよいワークスペースのクレート |
   |---|---|---|
   | `domain` | モデルと不変条件。純粋（I/O・非同期ランタイム・フレームワークなし） | なし |
   | `application` | ユースケースと、外部連携のポート（trait） | `domain` |
   | `infra` | ポートの実装（Postgres・モックの外部連携） | `domain`・`application` |
   | `interface` | axum の HTTP | `domain`・`application`（`infra` には直接依存しない） |

2. 依存方向は `cargo deny` ではなく、ワークスペース内のテスト `crates/architecture/tests/dependency_direction.rs` で検査する。`cargo metadata --no-deps` で各クレートの依存（通常・dev・build のすべて）を読み、上の表にない辺があれば落とす。表が規約の正本
   - 表にないクレートがワークスペースに増えても落とす（層を決めずにクレートを足させない）
   - domain の純粋さは、I/O 系の外部クレート（tokio・axum・hyper・tower・reqwest・sqlx・diesel・postgres など）への依存を禁じる一覧で検査する
   - domain が他の層に依存すると循環になるので、その場合は検査より先に cargo 自体が `cyclic package dependency` で落ちる
3. 検査は `cargo test` に含まれるので、CI の `cargo test` がそのまま依存方向のゲートになる（L1）。CI のコマンドは `apps/backend/README.md`
4. `interface` の `GET /healthz` は死活監視用で、student-app の契約（OpenAPI）には載せない
5. 当面は HTTP サーバーの起動（`main`）を `interface` に置く。infra の実装を組み立てる必要が出たら（T04 以降）、`interface` と `infra` の両方に依存してよい組み立て用のクレート（composition root）を足し、この表に 1 行加える

## 意図的に違反させて落ちることの確認（2026-10-26）

`apps/backend/` で次の変更を 1 つずつ入れて `cargo test -p architecture` を走らせ、すべて非 0 で落ちることを確かめてから元に戻した。

| 入れた違反 | 結果 |
|---|---|
| `interface` の `[dependencies]` に `infra.workspace = true` | `workspace_crates_follow_layer_dependency_direction` が失敗: `interface -> infra （Normal）は許されない` |
| `interface` の `[dev-dependencies]` に `infra.workspace = true` | 同じテストが失敗: `interface -> infra （Development）は許されない` |
| `domain` の `[dependencies]` に `tokio.workspace = true` | `domain_has_no_io_dependencies` が失敗: `依存してはならない: ["tokio"]` |
| `domain` の `[dependencies]` に `application.workspace = true` | cargo が `cyclic package dependency` で失敗（テストまで進まない） |

## 選ばなかった案

- **`cargo deny`（`[bans]` の `deny` / `wrappers`）**: 外部クレートの禁止には向くが、ワークスペース内のパス依存の辺を「このクレートからだけ許す」形で書くのが回りくどく、ワーカーのイメージに `cargo-deny` を足す必要もある。テストなら `cargo test` だけで済み、違反の文言も規約の言葉で出せる
- **層をモジュールで分けて 1 クレートにする**: ビルドは速いが、依存方向をコンパイラで強制できず、`use` の検査を独自に書くことになる
- **interface から infra への依存を許し、規約だけで縛る**: 機械で落とせず、L1 の検査にならない
