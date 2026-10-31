# ADR 0007: messaging の最小形（生徒と担当コーチのチャット）

- 状態: 提案（2026-10-27、D01 / Issue #90）。D47（Issue #207）でメッセージの種類と、添削依頼・添削の返却をスレッドに流すこと（決定 7）を追記。D48（Issue #208）で種類と送り手のシステムを契約に出すこと（決定 8）を追記。D54（Issue #227）でスタンプ（決定 9）を追記
- 関係: docs/adr/0009 の答え合わせ（チャットは会話でありレッスンのログ）、`specs/context-map.yaml`（messaging・async-lesson）、`specs/glossary.yaml`（message_thread / message / stamp / subscription）、`specs/contracts/events/asyncapi.yaml`（BookingConfirmed）、`specs/contracts/api/student-app/openapi.yaml`・`admin-web/openapi.yaml`（`listMessageThreads` / `listMessages` / `sendMessage` / `markMessageThreadRead`）、`specs/analytics/events.yaml`（message.sent）、ADR 0004（スキーマ）、ADR 0005（境界間イベントの購読）

## 決定

1. **スレッドは予約の確定で開く。** messaging は境界間イベント `BookingConfirmed` を購読し（`MessageThreadSubscriber`）、その生徒とコーチのスレッドを開く。生徒とコーチの組に 1 つで（domain の保存先のポートと DB の `threads_student_id_coach_id_key`）、同じ組の 2 件目の確定や同じイベントの再送では開き直さない。Gen1 は担当コーチを確定する matching がないので、**確定した予約のコーチを担当コーチとみなす**。予約をキャンセルしてもスレッドは閉じない。スレッドを開く HTTP の操作は置かない
2. **生徒が送ってよいかは entitlement に問い合わせる。** messaging のポート `ChatGate`（実装は `entitlement::EntitlementChatGate`）が、生徒の受講権が動画とチャットの送り放題（`Entitlement::unlimited_video_and_chat`。規則は `ContractType::unlimited_video_and_chat` の 1 か所）を持つかを答える。照会だけで、回数は消費しない（送り放題）。受講権のない生徒（月額契約の前）とワンショットの生徒は送れず、API は 403 `chat_not_entitled`（契約の 403 は「受講権がない、または役割が操作に合わない」）。コーチの送信は生徒の契約に左右されない（体験予約だけの生徒にもコーチは送れる）
3. **集約はスレッド。** `MessageThread` がメッセージを送った順に持ち、送信（当事者だけ・送り放題の判定・送信時刻はスレッドを開いた時刻と前のメッセージより前にならない）と既読の不変条件を持つ。メッセージの送り手はアカウントではなく、スレッドのどちらの当事者か（`MessageSender`: 生徒 / コーチ）で持つ（送り手が当事者であることが型で決まる）
4. **既読はメッセージごとの時刻。** `markMessageThreadRead` で、相手から届いたメッセージをすべてその時刻で既読にする（自分が送ったもの・既読のものは変えない。何度呼んでもよい）。メッセージの一覧を読むだけでは既読にしない。スレッドの一覧の `unread_count` は呼んだアカウントにとっての未読の数
5. **student-app と admin-web で同じ操作・同じ形。** 役割では絞らず、スレッドの当事者でないアカウントには 404（スレッドがあることも知らせない）。行動イベント `message.sent` は生徒・コーチどちらの送信でも発火し、送り手は `sender_role`（`student` / `coach`）に持つ。events.yaml の `actor` は 1 つしか書けないので代表として `student` にした
6. **ページングは置かない。** スレッドの一覧・メッセージの一覧は全件を返す（Gen1 のデモの規模。件数が増えたら契約に `limit` / カーソルを足す。後方互換に足せる）
7. **メッセージには種類があり、添削依頼と添削の返却がスレッドに流れる（D47）。** チャットは会話でありレッスンのログでもある（docs/adr/0009 の答え合わせ）。
   - 種類（domain の `MessageContent`。名前は `MessageKind`）は 3 つ: 本文（`Text`。本文を持つのは本文の種類だけ）、添削依頼（`VideoReviewRequest`。動画の ID）、添削の返却（`VideoReviewReturn`。動画の ID・添削の ID）。スタンプや AI コンシェルジュの発言は変種を足して表す
   - 送り手（`MessageAuthor`）はスレッドの当事者（`MessageSender`: 生徒 / コーチ）かシステム（`System`）。添削依頼の送り手は生徒、添削の返却の送り手はコーチだけ（`MessageContent::allows`。DB は `messages_kind_sender`）。システムのメッセージはどちらの当事者のものでもないので未読に数えず、既読にもしない。今はシステムのメッセージを流す処理はない（契約での返し方は決定 8）
   - async-lesson は動画の送付で境界間イベント `VideoReviewRequested` を、添削の返却で `VideoReviewReturned` を発行する。messaging の購読（`MessageThreadSubscriber`）がこの 2 つで、その生徒とコーチのスレッドに（なければイベントの `occurred_at` に開いて）メッセージを足す（送信時刻は `occurred_at`）。async-lesson は messaging を直接呼ばない
   - 同じ動画の添削依頼・添削の返却はスレッドに 1 通ずつ（`MessageThread::post_video_review_request` / `post_video_review_return`。DB は `messages_one_per_video_and_kind`）で、同じイベントが 2 回届いても 1 通。添削依頼のメッセージがない動画（この機能より前に送った動画）の返却も足す
   - メッセージは動画・添削を ID で指すだけで、添削の有無・回数・状態は async-lesson だけが持つ（messaging は写しを持たない）
   - 添削依頼・返却はチャットの送り放題の判定（`ChatGate`）を通さず、`MessageSent` と行動イベント `message.sent` も出さない（当事者がチャットで送ったものではない。返却の通知は notification が `VideoReviewReturned` から送る）
   - D47 の時点では API の形を変えず、本文に短い説明（「動画の添削を依頼しました」「動画の添削を返しました」。文言は interface）を入れて返した。種類は決定 8 で契約に出した
8. **メッセージの種類と送り手のシステムを契約に出す（D48）。** student-app・admin-web の `Message`（`listMessages`・`sendMessage`・スレッドの `last_message`）に、必須の `kind`（`MessageKind`: `text` / `video_review_request` / `video_review_returned`）と、種類ごとの中身（`video_id`: 添削依頼・返却のときだけ、`review_id`: 返却のときだけ）を足した。対応は interface の `contract.rs` の 1 か所
   - **知らない種類は読み飛ばしてよい**と契約（`MessageKind` と `listMessages`）に書いた。種類（スタンプなど）は後方互換に増やす。student-app は知らない種類のメッセージを一覧から除き、`last_message` なら null と同じに扱う
   - `body` は必須のまま残し、本文以外の種類では D47 の短い説明を入れ続ける。互換ポリシー（N-2）で支える古い版は `kind` を知らず `body` を必ず読むので、外すと古い版の表示が空になる（必須の項目を任意にするのは契約の違反）。知っている種類は説明に頼らず `kind` と ID で表す
   - 送り手のシステムは `sender_role` の `system`（レスポンスの enum の追加）で表し、`sender_id` はどのアカウントでもない nil UUID（`00000000-0000-0000-0000-000000000000`）にする。`sender_id` は必須の `string`（uuid）で、null を許すのは型の変更として契約の違反になるため。これで送り手がシステムのメッセージも 500 にならずに返る
   - `sendMessage` で送れるのは本文だけ（応答の `kind` は `text`）。リクエストは変えない
9. **スタンプを送れる（D54）。** オーナーの方向（2026-10-29: チャットには将来スタンプや AI コンシェルジュの発言も流せるよう、メッセージの種類をモデル化しておく）の 1 つ目。
   - スタンプ（glossary の `stamp`。domain の `Stamp`）は決まった 8 種類（`thanks` / `good` / `got_it` / `lets_go` / `nice_shot` / `tired` / `question` / `ok`）のどれかで、本文は持たない。種類 `MessageContent::Stamp`（`MessageKind::Stamp`）として足し、送り手は当事者（生徒・コーチ）だけ（`MessageContent::allows`。DB は `messages_kind_sender`）。画像と表示の文言はクライアントが持つ
   - 送信は本文と同じ `MessageThread::send` で、中身（`MessageContent`）を受け取る。チャットとして送れるのは本文とスタンプだけ（`MessageKind::is_chat`。添削依頼・返却は `MessagingError::MessageKindNotSendable`）。チャットの送り放題の判定（`ChatGate`）・`MessageSent`（相手への通知）・行動イベント `message.sent` は本文と同じで、属性も変えない（本文を載せないので種類も載せない）
   - 契約: `sendMessage` の `SendMessageRequest` に `stamp_id`（`StampId`）を足し、`body` を任意にした（リクエストの必須を外すのは互換）。本文とスタンプのどちらか 1 つで、両方・どちらもなし・知らない ID は 422。`Message` に `stamp_id`（スタンプのときだけ）を足し、`MessageKind` に `stamp` を足した（レスポンスの enum の追加。決定 8 で知らない種類は読み飛ばしてよいと書いてある）
   - `body` には古い版のクライアント向けに「スタンプ: ありがとう」の形の短い説明を入れる（呼び名は interface の `message.rs`。契約の `StampId` の説明と同じ）
   - DB は `messaging.messages` に `stamp` の列を足し、種類と形と送り手の CHECK を stamp の場合を OR で足すだけの付け直しにした（`20261029000002_expand_message_stamps.sql`）

## 選ばなかった案

- スレッドを開く操作（`POST /message-threads`、生徒がコーチを指定）: 誰とでもスレッドを作れてしまい「担当コーチ」の意味が残らない。Issue の操作（一覧・メッセージの一覧・送信・既読）からも外れる
- 動画の送付（`POST /videos`）でもスレッドを開く: D01 では、動画は境界間イベントを発行しておらず、送り先のコーチの存在も確かめていないので選ばなかった（`apps/backend/README.md` の async-lesson）。D47 で添削依頼をチャットに流すことにしたので、添削依頼・返却のイベントでスレッドがなければ開く（決定 7）。担当コーチの判断は matching を実装するときにまとめて見直す
- 添削依頼・返却の送り手をシステムにする（D47）: 契約の `sender_id` / `sender_role` に当てるアカウントがなく、今の応答の形を保てない。未読・既読もどちらの当事者のものか決まらない。依頼は生徒、返却はコーチの行為なので当事者を送り手にした
- 添削依頼を本文のメッセージに動画を添付する形にする（D47）: docs/adr/0009 の答え合わせ（チャットのメッセージの 1 つとして動画を上げるのではなく、添削依頼の形で上げさせる。添削したことや回数は管理したい）に合わない
- 種類ごとの中身を `oneOf`（`kind` で判別する別々のスキーマ）にする（D48）: 型は締まるが、今の `Message` を置き換えることになり互換の検査と両クライアントの型の扱いが重くなる。中身は ID が 2 つだけなので、任意の項目を平らに並べた
- 送り手がシステムのとき `sender_id` を null にする（D48）: 意味は素直だが、必須の uuid の項目に null を足すのはレスポンスの型の変更で、支えている古い版を壊す
- スタンプを本文のメッセージ（絵文字や決まった文字列）として送る（D54）: 種類として区別できず、絵柄の差し替え・集計・AI コンシェルジュなど後の種類の足し方の前例にならない。決定 7 の「変種を足して表す」に従った
- スタンプの送信を別の操作（`POST .../stamps`）にする（D54）: Issue の「`sendMessage` で本文の代わりにスタンプの ID を送る」に合わない。操作が増えると送り放題の判定・通知・行動イベントの経路が 2 つになる
- `SendMessageRequest` を `oneOf`（本文のスキーマとスタンプのスキーマ）にする（D54）: 型は締まるが、既存のスキーマの置き換えになり、互換の検査と両クライアントの型の扱いが重くなる（決定 8 の `Message` と同じ理由）。どちらか 1 つは API が 422 で守る
- 受講権のない生徒にも送らせる: Blueprint の「月額: 動画とチャットは送り放題」を、チャットは月額の利用権に含まれると読んだ。体験予約だけの生徒に送らせるかは、通知・matching と合わせて見直せる（`EntitlementChatGate` の 1 か所を変えればよい）
- 既読をスレッドの当事者ごとの「最後に読んだ時刻」で持つ: 行が少なく済むが、メッセージごとの既読（相手が読んだか）を返すには時刻の比較が要り、同じ時刻のメッセージで曖昧になる
