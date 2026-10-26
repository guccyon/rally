# Rally プロダクトの前提（Blueprint の要点）

工場のワーカーと人間が設計判断の前に読む 1 枚。正本は Blueprint v0.4（プロジェクトの資料フォルダ `10_blueprint-rally.yaml`）。ここは Blueprint の「何を作るか・制約」の写しで、実装方法は書かない。食い違ったら Blueprint が正。

## 事業の形

- 生徒とコーチをつなぐ C2C マーケットプレイス型のオンラインレッスン。スポーツはテニス・ゴルフ・野球・サッカー・バスケットボール
- レッスンは 2 種類。(1) ビデオ通話によるマンツーマンの**同期レッスン**（live lesson）。(2) 生徒が動画を送り、コーチが添削して返す**非同期レッスン**（async lesson / video review）
- 課金は 2 種類。**月額（subscription）**: ゴルフ以外のスポーツ。動画とチャットは送り放題、ビデオ通話は暦月 2 回まで。**ワンショット（one_shot）**: ゴルフだけ。動画添削 1 回ごとの都度課金
- 外部連携（Stripe / Twilio Video / S3 直接アップロード / 行動トラッキング / プッシュ通知）は Gen1 ではすべてモック。契約（ポート）は本物と同じ形

## Gen1 の範囲

- 生徒アプリ 1 本（iOS / Android、ゴルフ以外）＋最小の admin-web。コーチ側の最小操作（添削の返却・予約枠の登録・通話参加）は admin-web に同居
- 一周シナリオ: 体験予約 → 初回レッスン（通話 or 動画送付）→ 完了記録
- entitlement は月額とワンショットの両方を型として持ち、Gen1 で実装するのは月額のみ
- 契約（contracts/）は 5 アプリ分の境界を最初から引く

## 境界づけられたコンテキスト（初期案。`context-map.yaml` が正）

identity / catalog / matching / scheduling / live-lesson / async-lesson / messaging / entitlement / billing / review / notification / analytics。Gen1 のコアは identity / scheduling / entitlement。

## 不変条件の種（L4 の「所在集中度」の検査対象）

1. 月額契約の生徒のビデオ通話は暦月 2 回まで。3 回目は予約できない
2. ゴルフアプリの添削は、ワンショット決済が確定した動画にしか着手できない
3. キャンセル規定（開始 24 時間前以降のキャンセルは回数を消費）は scheduling と entitlement のどちらか一方で判定する（判定は scheduling。ADR 0002）
4. コーチへの支払い額は確定したレッスンのみから算出し、手数料率は契約種別ごとに固定

## 用語の注意

- 「受講権（entitlement）」は契約種別（月額 / ワンショット）と利用権（残回数・送り放題）の総称。月額では予約のたびに消費されるものではない。消費されるのはビデオ通話の月 2 回の枠だけ
- 「スロット（slot）」はコーチが公開する予約枠。副業コーチが枠をなかなか開けないことが事業上の課題
- 「予約（booking）」はスロットの確保。体験予約もこれ
- 「レッスン（lesson）」はレッスン種別（catalog）を指し、完了の事実は「完了したレッスン（lesson_completion）」と呼ぶ。完了したレッスンは live-lesson と async-lesson の両方が発行する（docs/adr/0006）

## Question への回答（2026-10-26、docs/adr/0002）

- 通話品質スコア: `call.ended` の 4 属性（packet_loss_rate / jitter_ms / round_trip_ms / disconnect_count）からの減点式。式はセマンティック層（analytics/kpi.yaml）に 1 か所
- キャンセル規定: 判定は scheduling（24 時間前以降か）、消費の実行は entitlement（BookingCancelled の consumes_quota を購読）
- コーチ側の最小操作: Gen1 は admin-web に「コーチ」ロールで同居。coach-web / coach-app は契約の枝だけ残す
