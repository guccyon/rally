# specs/contracts/

クライアントとバックエンド、境界づけられたコンテキスト同士の契約の置き場所。Blueprint v0.4 の「契約（contracts/）は 5 アプリ分の境界を最初から引く」に従い、Gen1 で実装しないアプリの分も枝だけ先に置く。本体は `api/student-app/openapi.yaml`（Gen1 一周シナリオの最小 API）から書き始めている。

## 契約の種類

| 種類 | 形式 | 置き場所 |
|---|---|---|
| クライアントが呼ぶ API | OpenAPI | `api/<アプリ名>/` |
| 境界づけられたコンテキスト間のドメインイベント | AsyncAPI | `events/` |

- 外部連携（Stripe / Twilio Video / S3 / 行動トラッキング / プッシュ通知）は Gen1 ではモックだが、契約（ポート）は本物と同じ形で書く
- 行動イベント（分析用のトラッキング）は契約ではなく `specs/analytics/` に置く

## ディレクトリの規約

```
contracts/
├── api/
│   ├── student-app/   # Gen1 で実装（openapi.yaml）
│   ├── admin-web/     # Gen1 で実装
│   ├── coach-app/     # 境界のみ
│   ├── coach-web/     # 境界のみ
│   └── golf-app/      # 境界のみ
└── events/            # AsyncAPI（境界間イベント）
```

- `api/` の枝はクライアントアプリ単位で切る。1 アプリ 1 枝
- イベントの発行元・購読先は `specs/context-map.yaml` の context id で書く
- 本体を書くまでは `.gitkeep` だけを置く
- OpenAPI は 3.1。スキーマ名は `specs/glossary.yaml` の id に揃え（booking → `Booking`）、各操作に `x-context`（context-map.yaml の id）を付ける。操作が発火する行動イベントは `x-analytics-events` に `specs/analytics/events.yaml` の name で書く
- 互換ポリシー（下記）は各 OpenAPI の `info.description` にも書く

## アプリと Gen1 の範囲

| アプリ | Gen1 | 備考 |
|---|---|---|
| student-app | 実装する | 生徒アプリ（iOS / Android、ゴルフ以外） |
| admin-web | 実装する | 最小の admin-web。コーチ側の最小操作（添削の返却・予約枠の登録・通話参加）も同居させる案（Question として未決） |
| coach-app | 境界だけ引く | |
| coach-web | 境界だけ引く | |
| golf-app | 境界だけ引く | ゴルフのワンショット課金のアプリ |

境界だけ引くアプリは枝を置くだけで、Gen1 では契約を書かない。

## 互換ポリシー

`.factory-config/manifest.yaml` の `compat.api` と一致させる。

- `min_supported_mobile_version: N-2`: モバイルアプリは最新から 2 つ前の版までを API が支える
- `deprecation_period_days: 90`: 廃止する API は deprecated にしてから 90 日は残す
