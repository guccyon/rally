# specs/stories/

ユーザーストーリー（利用者の流れ）の正本。QA が保証するのは機能の正しさより「利用者の流れ」で、そのための入力になる（factory-engine の ADR 0028）。型は factory-engine の `definitions/user_story.yaml`。

- 1 本 1 ファイル（`<id>.yaml`）。`type: UserStory` を付ける
- `actors.yaml`: 利用者の種類（生徒・コーチ・本部・CS）。ストーリーの `actor` / `reader` / `steps[].by` はここの id
- 書くもの: 誰が（`actor`）・何のため（`goal`）・読み手（`reader`）・入口（`entry`）と出口（`exit`）・流れ（`steps`。1 歩ごとに `where` / `do` / `sees`）・してはいけないこと（`must_not`）。任意で前提のデータ（`preconditions`）・時間や再起動をまたぐか（`time`）・出てくる機能（`touches`）・起点（`origin`）・今の実装が満たしていないところ（`known_gaps`）
- ストーリーは「あるべき流れ」で書く。既存の機能に遡って書いて、今の実装が満たしていないところは `known_gaps` に書く（直したら消す）
- `touches` には正本にある id だけを書く（`context-map.yaml` の id、`glossary.yaml` の id、`<app>.<operationId>`、`analytics/events.yaml` の name）

検査は `apps/backend/crates/application/tests/user_stories.rs`（必須の欄があること、id とファイル名が揃うこと、`actors.yaml` と `touches` の id が正本にあること）。

## いまのストーリー

| id | 誰が | 流れ |
|---|---|---|
| `coach-replies-to-new-message` | コーチ | 生徒から届いた新しい相談に気づいて返事する（#895。未返信の印は #1048） |
| `student-subscription-persists` | 生徒 | 月額契約を始め、翌日アプリを開き直しても契約が残っている（入口はホームの「次にやること」・今月のビデオ通話・プロフィール。#1436） |
| `cs-traces-vanished-booking` | CS | 「予約が消えた」という問い合わせに、予約の経緯を追って答える |
| `student-prejoin-check` | 生徒 | 開始 5 分前から入室前チェックでカメラの映りと背景・マイク・スピーカーを確かめ、開始時刻からビデオ通話（モック）に入る（#1039） |
| `student-books-after-first-lesson` | 生徒 | 初回レッスンを受け終えたあと、ホームの「次のレッスンを予約する」から 2 回目の予約に進む（#1360） |
| `student-resumes-next-action` | 生徒 | 予約やレッスンの途中で画面を離れても、ホームの「次にやること」（サーバーの状態から出す）から同じ次の操作に戻れる（#1436） |
| `coach-prejoin-check` | コーチ | 開始 5 分前から入室前チェックでカメラの映りと背景・マイク・スピーカーを確かめ、開始時刻からビデオ通話（モック）に入る（#1039） |
