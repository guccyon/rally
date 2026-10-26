# specs/contracts/

アプリと境界をまたぐ契約の置き場。5 アプリ分の境界を最初から引き、中身（OpenAPI / AsyncAPI の本体）は必要になった時点で書く。

## 契約の種類

- **API**: OpenAPI。クライアント（アプリ）とバックエンドの間
- **ドメインイベント**: AsyncAPI。境界づけられたコンテキスト間（`../context-map.yaml` の id）。イベントの例は `events/README.md`

## ディレクトリの規約

```
contracts/
├── api/
│   ├── student-app/   # 生徒アプリ
│   ├── admin-web/     # 管理 Web（Gen1 ではコーチ側の最小操作も同居）
│   ├── coach-app/     # コーチアプリ
│   ├── coach-web/     # コーチ Web
│   └── golf-app/      # ゴルフアプリ
└── events/            # 境界間ドメインイベント（AsyncAPI）
```

- クライアントごとに `api/<アプリ名>/` を 1 ディレクトリ持つ。空の枝は `.gitkeep` で保持する
- 用語は `../glossary.yaml`、コンテキスト名は `../context-map.yaml` に合わせる

## 互換ポリシー

- `min_supported_mobile_version` は N-2（現行から 2 世代前まで）をサポートする
- 廃止（deprecation）の予告から削除まで 90 日を置く

## Gen1 の実装範囲

- 実装するクライアント: **student-app**、**admin-web**
- 境界だけ引く（Gen1 では実装しない）: **coach-app**、**coach-web**、**golf-app**
