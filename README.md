# Rally

株式会社コートサイド（架空）のオンラインスポーツレッスン「Rally」の Product Monorepo。
自律型ソフトウェア事業部（Factory of Factories）の 1 号機（PoC）で、[guccyon/factory-engine](https://github.com/guccyon/factory-engine) をマウントして動く工場兼プロダクトになる。

> 企業名・数値・現状指標はすべて架空。外部連携（Stripe / Twilio / S3 / 行動トラッキング / プッシュ通知）は Gen1 ではすべてモック。

- 現在の Phase: **P0（計測基盤）Week 1**。工場の設定の骨格だけがあり、アプリコードはまだない（P1 入口のブートストラップで Walking Skeleton を立ち上げる）
- Gen1 の範囲: 生徒アプリ 1 本 ＋ 最小の admin-web。一周シナリオは「体験予約 → 初回レッスン → 完了記録」

## 構成

| パス | 役割 | 状態 |
|---|---|---|
| `.factory-config/manifest.yaml` | 憲法レベルの設定。autonomy（環境 × 変更種別）・予算・互換 | P0 起案（署名待ち） |
| `.factory-config/base/` | Engine が生成する設定（編集禁止）。P0〜P5 は Engine の版を指すだけ | 骨格 |
| `.factory-config/overlay/` | 学習で育つルール。各ルールに confidentiality（product / client / public）必須。P0 は書き込まない | 骨格 |
| `.factory-config/evals/` | この製品の Golden Set とルール検証。`golden/` にペア比較の採点を置く | 骨格 |
| `specs/` | SSOT（glossary・context-map・db-schemas・contracts・analytics） | P1 |
| `apps/` | backend（Rust / axum）・student-app（Flutter）・admin-web（Next.js） | P1 |
| `docs/adr/` | 設計判断の記録 | 随時 |
| `docs/plans/` | Phase ごとの実行計画（P1 入口の Walking Skeleton: `p1-walking-skeleton.md`） | Phase 境界 |

## Blueprint

この工場の入力は Blueprint（`blueprint_version: 0.4`）。正本はプロジェクトの資料フォルダにあり、決定は `decisions:` に日付付きで追記する。
`manifest.yaml` の autonomy・budgets・compat は Blueprint の同名の節と一致させる。

## 利用について

この工場は「仮想開発事業部」の 1 号機で、実行には Engine の実行体を含む。工場で育った固有の知識（`.factory-config/overlay/` `evals/`）はプロダクト側に帰属し、Engine の知財は建設者に帰属する（factory-engine ADR 0001）。開発中のため public で公開しているが、ライセンスはまだ定めていない。
