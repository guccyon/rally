# Rally の開発ガイド（エージェント向け）

工場のワーカーと人間の手元の Claude が共通に守ること。設計の正本は `specs/`（最初に `specs/product-context.md`）、判断は `docs/adr/`。

## 書き方

- 文書・コメント・PR の説明・コミットメッセージは日本語。識別子・コマンド・ツールの出力は原文のまま
- PR の説明は 概要 / 対応内容 / 影響範囲・互換性 / 動作確認 / 補足 / 知見 の見出しで。該当しない見出しは省く
- コード・テスト・文書は同じ PR に入れる。利用者や API の利用者に見える変更は `CHANGELOG.md` の `[Unreleased]` に 1 行足す

## バックエンド（apps/backend）

- 層と依存の向きは `docs/adr/0003-backend-layers.md`。domain は外部クレートに依存しない。interface は infra に依存しない（infra の実装を組み立てるのは server）
- 契約・DB の文字列表現や、利用者向けのメッセージは domain に置かない（interface / infra のマッパーで）
- 変換は 1 か所で。ワークスペースにあるクレート（uuid など）があるものを自前で書かない
- 1 ファイル 1 公開型。テストは本体と同じ層の構成に
- コミット前に `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` が通ること。DB が要るテストは環境変数 `DATABASE_URL` の Postgres に向ける（ワーカーにはジョブ専用の空の DB が渡される。自前で Postgres を立てない）
- DB の変更は expand → 移行 → contract（`docs/adr/0004-schema-expand-contract.md`）。contract は人間の承認

## 用語と境界

- 用語は `specs/glossary.yaml`、コンテキストは `specs/context-map.yaml` の id を使う。推測で定義を作らない
- 行動イベントは `specs/analytics/events.yaml`、境界間イベントは `specs/contracts/events/asyncapi.yaml`、API は `specs/contracts/api/<app>/openapi.yaml` が正。実装はこれに合わせる

## やってはいけないこと

- 秘密情報・権限設定・決済ロジック・破壊的スキーマ変更に触れること（必要なら作業を止めて理由を書く）
- `.github/workflows/` の変更（ワーカーの権限にない。建設者が行う）
