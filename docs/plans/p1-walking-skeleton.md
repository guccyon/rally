# P1 入口のブートストラップ: Walking Skeleton の計画（2026-10-26 起案）

開始定義 第 3 節 Week 5〜 と憲章 07 節 P1 の「建設者が手書きの 15〜20 本のチケットで Rally のコア 2〜3 コンテキスト（identity / scheduling / entitlement）の Walking Skeleton を立ち上げ、一周シナリオ『体験予約 → 初回レッスン → 完了記録』を通す」の実行計画。チケットは工場（Issue ラベル → ワーカー → PR）に流す。

## 前提（Blueprint v0.4 と specs/ から）

- バックエンド Rust / axum / PostgreSQL、Web は TypeScript / Next.js、モバイルは Flutter。契約は OpenAPI（API）と AsyncAPI（境界間イベント）
- Gen1 は student-app ＋最小の admin-web。外部連携はすべてモック。entitlement は月額とワンショットの両方を型として持ち、実装は月額のみ
- コアの 3 コンテキスト: identity（アカウント・権限）/ scheduling（スロット・予約・キャンセル）/ entitlement（契約種別・利用権）。一周シナリオには live-lesson（通話の開始・終了の記録）と async-lesson（動画の送付・添削の返却）の最小の記録も要る
- 計測は `specs/analytics/events.yaml` のイベントを発火することで取る（モックのトラッキングに流す）

## 段（契約ファースト。段 1 をマージしてから段 2 以降を並列に）

| 段 | 目的 | チケット |
|---|---|---|
| 0 | 工場側の準備 | ワーカーに Rust ツールチェーンを持つイメージ（factory-engine）。rally の CI（`cargo fmt --check` / `cargo clippy` / `cargo test`）と main の必須チェック |
| 1 | 骨格と契約 | T01 Cargo ワークスペースと層の規約（domain / application / infra / interface、依存方向テスト）。T02 OpenAPI: student-app の体験予約〜完了記録の最小 API（契約だけ）。T03 AsyncAPI: 境界間イベント 3 つ（BookingConfirmed / LessonCompleted / SubscriptionStarted）。T04 DB スキーマ初版（expand/contract の規約、マイグレーション基盤） |
| 2 | identity | T05 生徒・コーチ・本部アカウントのモデルと不変条件。T06 モックの認証（トークン発行・検証のポート、裏はモック） |
| 3 | scheduling | T07 スロットの公開と検索。T08 予約の作成・確定（体験予約を含む）。T09 キャンセル規定（24 時間前以降は回数を消費。判定の置き場所は Question に従う） |
| 4 | entitlement | T10 契約種別（月額 / ワンショット）と利用権の型。T11 月額契約の開始と、ビデオ通話の暦月 2 回の枠の消費（不変条件 1） |
| 5 | 一周の接続 | T12 通話の開始・終了の記録（live-lesson の最小。品質指標はモックが生成）。T13 動画の送付と添削の返却の記録（async-lesson の最小）。T14 レッスンの完了記録と is_first_lesson。T15 行動イベントの発火（events.yaml どおり。モックのトラッキングへ） |
| 6 | 外側 | T16 admin-web の最小（コーチ側の枠登録・添削返却・通話参加）。T17 student-app の最小画面（Flutter。予約〜完了まで）。T18 合成シード v0（コーチ 30・登録 1000・MAU 100・12 ヶ月。偏りは Blueprint synthetic_seed） |
| 7 | 一周の確認 | T19 一周シナリオの E2E（API レベル）。T20 要求者ペルソナの初版と intake / clarify の最小経路（cs-1 が「最近テニスの生徒の反応が鈍い気がする」と聞いてくる） |

## 進め方

- 1 チケット 1 Issue。本文は目的 / 先に読むもの / やること / やらないこと / 受け入れ条件（テストで確かめられる形）。`factory:implement` を付けると工場が PR を出す
- 段 1（T01〜T04）は契約と型なので先にマージし、段 2〜4 は並列に流す（マージの直列化はコンテキストごと）
- 各 Issue は tier-large で 1 回。Golden Set のペアが要るものは tier-small でも 1 回走らせる
- L1（依存方向テスト、fmt、clippy）は T01 で CI に入れ、以後の PR は CI が落ちればマージしない。L2（ミューテーション）は Walking Skeleton が通ってから
- Blueprint の Question 3 件（通話品質スコアの定義、キャンセル規定の置き場所、admin-web 同居）は、該当チケット（T12 / T09 / T16）の前に決める。決めるのは建設者（委任を受けた Claude が案を出し、オーナー が否定しなければ採用）

## やらないこと（P1 では）

- iOS ビルド・TestFlight（P2）。Flutter は Web ターゲットか Android エミュレータで確認できる範囲まで
- 本番相当のインフラ（IaC は P2）。ローカルの Docker Compose（Postgres）だけ
- 自己進化（ルールの自動採用）。shadow と記録に留める
