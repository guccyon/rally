# ADR 0007: messaging の最小形（生徒と担当コーチのチャット）

- 状態: 提案（2026-10-27、D01 / Issue #90）
- 関係: `specs/context-map.yaml`（messaging）、`specs/glossary.yaml`（message_thread / message / subscription）、`specs/contracts/events/asyncapi.yaml`（BookingConfirmed）、`specs/contracts/api/student-app/openapi.yaml`・`admin-web/openapi.yaml`（`listMessageThreads` / `listMessages` / `sendMessage` / `markMessageThreadRead`）、`specs/analytics/events.yaml`（message.sent）、ADR 0004（スキーマ）、ADR 0005（境界間イベントの購読）

## 決定

1. **スレッドは予約の確定で開く。** messaging は境界間イベント `BookingConfirmed` を購読し（`MessageThreadSubscriber`）、その生徒とコーチのスレッドを開く。生徒とコーチの組に 1 つで（domain の保存先のポートと DB の `threads_student_id_coach_id_key`）、同じ組の 2 件目の確定や同じイベントの再送では開き直さない。Gen1 は担当コーチを確定する matching がないので、**確定した予約のコーチを担当コーチとみなす**。予約をキャンセルしてもスレッドは閉じない。スレッドを開く HTTP の操作は置かない
2. **生徒が送ってよいかは entitlement に問い合わせる。** messaging のポート `ChatGate`（実装は `entitlement::EntitlementChatGate`）が、生徒の受講権が動画とチャットの送り放題（`Entitlement::unlimited_video_and_chat`。規則は `ContractType::unlimited_video_and_chat` の 1 か所）を持つかを答える。照会だけで、回数は消費しない（送り放題）。受講権のない生徒（月額契約の前）とワンショットの生徒は送れず、API は 403 `chat_not_entitled`（契約の 403 は「受講権がない、または役割が操作に合わない」）。コーチの送信は生徒の契約に左右されない（体験予約だけの生徒にもコーチは送れる）
3. **集約はスレッド。** `MessageThread` がメッセージを送った順に持ち、送信（当事者だけ・送り放題の判定・送信時刻はスレッドを開いた時刻と前のメッセージより前にならない）と既読の不変条件を持つ。メッセージの送り手はアカウントではなく、スレッドのどちらの当事者か（`MessageSender`: 生徒 / コーチ）で持つ（送り手が当事者であることが型で決まる）
4. **既読はメッセージごとの時刻。** `markMessageThreadRead` で、相手から届いたメッセージをすべてその時刻で既読にする（自分が送ったもの・既読のものは変えない。何度呼んでもよい）。メッセージの一覧を読むだけでは既読にしない。スレッドの一覧の `unread_count` は呼んだアカウントにとっての未読の数
5. **student-app と admin-web で同じ操作・同じ形。** 役割では絞らず、スレッドの当事者でないアカウントには 404（スレッドがあることも知らせない）。行動イベント `message.sent` は生徒・コーチどちらの送信でも発火し、送り手は `sender_role`（`student` / `coach`）に持つ。events.yaml の `actor` は 1 つしか書けないので代表として `student` にした
6. **ページングは置かない。** スレッドの一覧・メッセージの一覧は全件を返す（Gen1 のデモの規模。件数が増えたら契約に `limit` / カーソルを足す。後方互換に足せる）

## 選ばなかった案

- スレッドを開く操作（`POST /message-threads`、生徒がコーチを指定）: 誰とでもスレッドを作れてしまい「担当コーチ」の意味が残らない。Issue の操作（一覧・メッセージの一覧・送信・既読）からも外れる
- 動画の送付（`POST /videos`）でもスレッドを開く: 動画は境界間イベントを発行しておらず、送り先のコーチの存在も確かめていない（`apps/backend/README.md` の async-lesson）。担当コーチの判断は matching を実装するときにまとめて見直す
- 受講権のない生徒にも送らせる: Blueprint の「月額: 動画とチャットは送り放題」を、チャットは月額の利用権に含まれると読んだ。体験予約だけの生徒に送らせるかは、通知・matching と合わせて見直せる（`EntitlementChatGate` の 1 か所を変えればよい）
- 既読をスレッドの当事者ごとの「最後に読んだ時刻」で持つ: 行が少なく済むが、メッセージごとの既読（相手が読んだか）を返すには時刻の比較が要り、同じ時刻のメッセージで曖昧になる
