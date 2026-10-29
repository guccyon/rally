# ADR 0007: messaging の最小形（生徒と担当コーチのチャット）

- 状態: 提案（2026-10-27、D01 / Issue #90）。D47（Issue #207）でメッセージの種類と、添削依頼・添削の返却をスレッドに流すこと（決定 7）を追記
- 関係: docs/adr/0009 の答え合わせ（チャットは会話でありレッスンのログ）、`specs/context-map.yaml`（messaging・async-lesson）、`specs/glossary.yaml`（message_thread / message / subscription）、`specs/contracts/events/asyncapi.yaml`（BookingConfirmed）、`specs/contracts/api/student-app/openapi.yaml`・`admin-web/openapi.yaml`（`listMessageThreads` / `listMessages` / `sendMessage` / `markMessageThreadRead`）、`specs/analytics/events.yaml`（message.sent）、ADR 0004（スキーマ）、ADR 0005（境界間イベントの購読）

## 決定

1. **スレッドは予約の確定で開く。** messaging は境界間イベント `BookingConfirmed` を購読し（`MessageThreadSubscriber`）、その生徒とコーチのスレッドを開く。生徒とコーチの組に 1 つで（domain の保存先のポートと DB の `threads_student_id_coach_id_key`）、同じ組の 2 件目の確定や同じイベントの再送では開き直さない。Gen1 は担当コーチを確定する matching がないので、**確定した予約のコーチを担当コーチとみなす**。予約をキャンセルしてもスレッドは閉じない。スレッドを開く HTTP の操作は置かない
2. **生徒が送ってよいかは entitlement に問い合わせる。** messaging のポート `ChatGate`（実装は `entitlement::EntitlementChatGate`）が、生徒の受講権が動画とチャットの送り放題（`Entitlement::unlimited_video_and_chat`。規則は `ContractType::unlimited_video_and_chat` の 1 か所）を持つかを答える。照会だけで、回数は消費しない（送り放題）。受講権のない生徒（月額契約の前）とワンショットの生徒は送れず、API は 403 `chat_not_entitled`（契約の 403 は「受講権がない、または役割が操作に合わない」）。コーチの送信は生徒の契約に左右されない（体験予約だけの生徒にもコーチは送れる）
3. **集約はスレッド。** `MessageThread` がメッセージを送った順に持ち、送信（当事者だけ・送り放題の判定・送信時刻はスレッドを開いた時刻と前のメッセージより前にならない）と既読の不変条件を持つ。メッセージの送り手はアカウントではなく、スレッドのどちらの当事者か（`MessageSender`: 生徒 / コーチ）で持つ（送り手が当事者であることが型で決まる）
4. **既読はメッセージごとの時刻。** `markMessageThreadRead` で、相手から届いたメッセージをすべてその時刻で既読にする（自分が送ったもの・既読のものは変えない。何度呼んでもよい）。メッセージの一覧を読むだけでは既読にしない。スレッドの一覧の `unread_count` は呼んだアカウントにとっての未読の数
5. **student-app と admin-web で同じ操作・同じ形。** 役割では絞らず、スレッドの当事者でないアカウントには 404（スレッドがあることも知らせない）。行動イベント `message.sent` は生徒・コーチどちらの送信でも発火し、送り手は `sender_role`（`student` / `coach`）に持つ。events.yaml の `actor` は 1 つしか書けないので代表として `student` にした
6. **ページングは置かない。** スレッドの一覧・メッセージの一覧は全件を返す（Gen1 のデモの規模。件数が増えたら契約に `limit` / カーソルを足す。後方互換に足せる）
7. **メッセージには種類があり、添削依頼と添削の返却がスレッドに流れる（D47）。** チャットは会話でありレッスンのログでもある（docs/adr/0009 の答え合わせ）。
   - 種類（domain の `MessageContent`。名前は `MessageKind`）は 3 つ: 本文（`Text`。本文を持つのは本文の種類だけ）、添削依頼（`VideoReviewRequest`。動画の ID）、添削の返却（`VideoReviewReturn`。動画の ID・添削の ID）。スタンプや AI コンシェルジュの発言は変種を足して表す
   - 送り手（`MessageAuthor`）はスレッドの当事者（`MessageSender`: 生徒 / コーチ）かシステム（`System`）。添削依頼の送り手は生徒、添削の返却の送り手はコーチだけ（`MessageContent::allows`。DB は `messages_kind_sender`）。システムのメッセージはどちらの当事者のものでもないので未読に数えず、既読にもしない。今はシステムのメッセージを流す処理はなく、契約にも形がない（返すことになれば 500。形は次の段で決める）
   - async-lesson は動画の送付で境界間イベント `VideoReviewRequested` を、添削の返却で `VideoReviewReturned` を発行する。messaging の購読（`MessageThreadSubscriber`）がこの 2 つで、その生徒とコーチのスレッドに（なければイベントの `occurred_at` に開いて）メッセージを足す（送信時刻は `occurred_at`）。async-lesson は messaging を直接呼ばない
   - 同じ動画の添削依頼・添削の返却はスレッドに 1 通ずつ（`MessageThread::post_video_review_request` / `post_video_review_return`。DB は `messages_one_per_video_and_kind`）で、同じイベントが 2 回届いても 1 通。添削依頼のメッセージがない動画（この機能より前に送った動画）の返却も足す
   - メッセージは動画・添削を ID で指すだけで、添削の有無・回数・状態は async-lesson だけが持つ（messaging は写しを持たない）
   - 添削依頼・返却はチャットの送り放題の判定（`ChatGate`）を通さず、`MessageSent` と行動イベント `message.sent` も出さない（当事者がチャットで送ったものではない。返却の通知は notification が `VideoReviewReturned` から送る）
   - API（`listMessages` / `listMessageThreads` の `last_message`）の形は変えず、当面は本文に短い説明（「動画の添削を依頼しました」「動画の添削を返しました」。文言は interface）を入れて、送り手の当事者を `sender_id` / `sender_role` で返す。種類を契約に出すのは次の段

## 選ばなかった案

- スレッドを開く操作（`POST /message-threads`、生徒がコーチを指定）: 誰とでもスレッドを作れてしまい「担当コーチ」の意味が残らない。Issue の操作（一覧・メッセージの一覧・送信・既読）からも外れる
- 動画の送付（`POST /videos`）でもスレッドを開く: D01 では、動画は境界間イベントを発行しておらず、送り先のコーチの存在も確かめていないので選ばなかった（`apps/backend/README.md` の async-lesson）。D47 で添削依頼をチャットに流すことにしたので、添削依頼・返却のイベントでスレッドがなければ開く（決定 7）。担当コーチの判断は matching を実装するときにまとめて見直す
- 添削依頼・返却の送り手をシステムにする（D47）: 契約の `sender_id` / `sender_role` に当てるアカウントがなく、今の応答の形を保てない。未読・既読もどちらの当事者のものか決まらない。依頼は生徒、返却はコーチの行為なので当事者を送り手にした
- 添削依頼を本文のメッセージに動画を添付する形にする（D47）: docs/adr/0009 の答え合わせ（チャットのメッセージの 1 つとして動画を上げるのではなく、添削依頼の形で上げさせる。添削したことや回数は管理したい）に合わない
- 受講権のない生徒にも送らせる: Blueprint の「月額: 動画とチャットは送り放題」を、チャットは月額の利用権に含まれると読んだ。体験予約だけの生徒に送らせるかは、通知・matching と合わせて見直せる（`EntitlementChatGate` の 1 か所を変えればよい）
- 既読をスレッドの当事者ごとの「最後に読んだ時刻」で持つ: 行が少なく済むが、メッセージごとの既読（相手が読んだか）を返すには時刻の比較が要り、同じ時刻のメッセージで曖昧になる
