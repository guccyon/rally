# 変更履歴

Keep a Changelog の形。利用者（生徒・コーチ・本部）や API の利用者に見える変更を `[Unreleased]` に足し、リリース時に版の項へ移す。工場のワーカーも、利用者に見える変更を含む PR ではここを更新する（rubric DOC-002）。

## [Unreleased]

### 追加
- 自分の予約の一覧の API: student-app の `GET /bookings/me`（`listMyBookings`）が、生徒本人の予約をキャンセル済みも含めて状態付きで、予約したスロット（日時・スポーツ。コーチの表示名と自己紹介文は任意）と一緒に、スロットの開始時刻の新しい順に返す。生徒以外は 403。DB の変更はない。student-app はモデルで読むだけで、画面はまだない（#149）
- student-app のスロットを探す画面で、各スロットにコーチの名前（「コーチ: 名前」）と自己紹介文を出す。自己紹介文は 2 行までで、収まらないときは「続きを読む」を押すと全文を出し、「閉じる」で戻す。名前や自己紹介文がないスロットは今までどおり。API と契約の変更はない（#139）
- スロットの検索にコーチの紹介: student-app の `GET /slots`（`searchSlots`）の各枠 `Slot` に、任意の項目 `coach_display_name`（コーチの表示名）と `coach_bio`（自己紹介文）を足した。自己紹介文を書いていないコーチの枠には `coach_bio` を載せない。既存の項目は変えない。DB の変更はない。student-app はモデルで読むだけで、画面はまだ変えない（#138）
- admin-web のコーチの画面に「自己紹介」: 開いたときに今の自己紹介文を出し、編集して保存する（`PUT /me/profile`）。残りの文字数を出し、400 文字を超えると保存しない。空にして保存すると消える。API と契約の変更はない（#137）
- コーチの自己紹介文の API: admin-web の `PUT /me/profile`（`updateMyCoachProfile`）でコーチが自分の自己紹介文（0〜400 文字。空文字列で消す）を置き換え、`GET /me` の応答にコーチのときだけ任意の項目 `bio`（書いていなければ空文字列）を足した。本部・生徒は 403、400 文字を超えると 422。既存の項目は変えない。DB は列の追加だけ（expand）。admin-web の画面はまだない（#136）
- student-app に通知の受け取り設定の画面: 登録後はアプリバーの「通知の設定」から開き、「予約が確定したとき」「動画の添削が返ってきたとき」「担当コーチからメッセージが届いたとき」のスイッチで受け取るかを切り替える。切り替えるとその種類だけをすぐ保存し（`PUT /notification-settings/me`）、保存に失敗したら元に戻して知らせる。API と契約の変更はない（#135）
- 通知の受け取り設定の API: student-app の `GET /notification-settings/me`（`getMyNotificationSettings`）と `PUT /notification-settings/me`（`updateMyNotificationSettings`）で、生徒が通知の種類（予約の確定・添削の返却・メッセージの受信）ごとに受け取るかを読み書きする（既定はすべて受け取る。PUT は書いた種類だけを変える。生徒以外は 403）。オフにした種類のプッシュ通知は送らず、送らなかった通知はあとでオンに戻しても送り直さない。DB は表の追加だけ（expand）。student-app の画面はまだない（#134）
- admin-web の本部の「コーチの評価」（`/headquarters/coach-review-summary`）で、コーチ ID の入力をやめ、開いたときに `GET /coaches` で取ったコーチの一覧（表示名）から選んで、件数と平均とその下に評価の一覧（`GET /coaches/{coach_id}/reviews`。コーチの「自分の評価」と同じ表示）を出す。API と契約の変更はない（#114）
- admin-web のコーチの「自分の評価」で、アカウント ID を入力せずに、開いたときから本人の評価の件数と平均と一覧を出す（`GET /me` で本人の ID を知る）。本部の画面は変わらない。API と契約の変更はない（#123）
- ログイン中の本人のアカウントの API: admin-web の `GET /me`（`getMyAccount`）がトークンの示すアカウントの ID・役割（`student` / `coach` / `headquarters`）・表示名を返す。役割では絞らず、トークンがない・無効なら 401（トークンのアカウントがない、または役割が合わないときも 401）。admin-web の画面はまだない（#122）
- student-app のレッスンの評価で、1〜5 の各点に言葉の目安（1「不満だった」〜5「とてもよかった」）を添え、選んだ点と送った後の評価も「4（よかった）」のように言葉付きで出す。API と契約の変更はない（#120）
- 予約の確定をコーチにもプッシュ通知: 予約が確定すると、生徒への「予約の確定」に加えて、予約を受けたコーチに「新しい予約」の通知（`BookingReceived`）を 1 件送る。同じイベントの再送では送らず、片方だけ送れなかったときは送れなかった方だけを再送で送る。送り先は Gen1 ではモック。API と契約の変更はない（#119）
- 本部向けのコーチの一覧の API: admin-web の `GET /coaches`（`listCoaches`）がコーチのアカウントの ID と表示名を、表示名の順（コードポイントの順、同じ表示名は ID の順）に返す。本部だけが読め、コーチ・生徒は 403。admin-web の画面はまだない（#113）
- admin-web にコーチの評価の一覧: コーチの「自分の評価」で、件数と平均の下に `GET /coaches/{coach_id}/reviews` の評価を 1 件ずつ（点・一言・送った時刻、新しい順）出す。0 件なら「まだ評価はありません」。本部の画面は変わらない。API と契約の変更はない（#112）
- コーチが受けた評価の一覧の API: admin-web の `GET /coaches/{coach_id}/reviews`（`listCoachReviews`）が評価の ID・完了したレッスンの ID・点・一言・送った時刻を、送った時刻の新しい順に返す。本部はどのコーチの分も、コーチは自分の分だけ読め、生徒は 403。admin-web の画面はまだない（#111）
- 添削の返却とメッセージの受信でプッシュ通知（notification の後半）: 境界間イベント `VideoReviewReturned`（async-lesson）と `MessageSent`（messaging）を AsyncAPI に足して発行し、notification が購読して、添削の返却は生徒へ、メッセージは受け手（送り手でない側）へ送る。送り先は Gen1 ではモック。API の変更はない（#100）
- admin-web にコーチの評価の件数と平均: コーチは「自分の評価」で自分のアカウント ID を入力して、本部は `/headquarters/coach-review-summary` でコーチ ID を指定して、`GET /coaches/{coach_id}/review-summary` の件数と平均（0 件なら「評価はまだありません」）を見る。API と契約の変更はない（#95）
- student-app にレッスンの評価の画面: 完了を記録したあと「レッスンを評価する」から 1〜5 の点と一言を送る。送ったあとと、すでに評価済み（409 `lesson_already_reviewed`）のときは入力できない。API と契約の変更はない（#94）
- student-app に担当コーチとのチャットの画面: アプリバーの「チャット」からスレッドの一覧（未読の数・最後のメッセージ）→ メッセージの一覧と送信。開いたスレッドの相手のメッセージは既読にし、自分が送ったメッセージには既読・未読を出す。月額契約の前に送ると「チャットは月額契約を始めると使えます。」と出す。API と契約の変更はない（#93）
- 予約の確定で生徒にプッシュ通知（notification の最小形・前半）: notification が境界間イベント `BookingConfirmed` を購読して送る。送り先の基盤は Gen1 ではモック（送った通知を記録し、標準出力に 1 行書く）。API の変更はない（#91）
- レッスンの評価（review の最小形）: 生徒が完了したレッスン 1 件につき 1 回、1〜5 の点と一言を付ける `POST /lesson-reviews`（2 回目は 409 `lesson_already_reviewed`）と、admin-web の本部とコーチ本人がコーチの評価の件数と平均を読む `GET /coaches/{coach_id}/review-summary`。評価で行動イベント `review.submitted` を発火（#92）
- 生徒と担当コーチのチャット（messaging の最小形）: student-app と admin-web の `GET /message-threads`・`GET /message-threads/{thread_id}/messages`・`POST /message-threads/{thread_id}/messages`・`POST /message-threads/{thread_id}/read`。スレッドは予約の確定で生徒とコーチの組に 1 つ開く。月額の生徒は送り放題（回数を消費しない）、月額契約の前の生徒の送信は 403 `chat_not_entitled`。送信時刻と既読を返し、送信で行動イベント `message.sent` を発火（#90）
- admin-web の本部の画面に「スポーツ別の予約枠の需給」（`/headquarters/slot-supply`）: `GET /slot-supply` の結果をスポーツごとに「足りている / 足りていない」と、公開した枠の数・予約しようとした生徒の数で一覧にし、集計の期間を出す。本部以外のロールでは入れず、API の 403 はその旨を表示（#85）
- 行動イベントの共通の任意属性 `release_id`（版の ID。例: `guccyon/rally@0.1.0`）と `app_version`（アプリの版）を `specs/analytics/events.yaml` の `common_optional` に定義。`ANALYTICS_SINK=object_store` の配信先は、環境変数 `ANALYTICS_RELEASE_ID` / `ANALYTICS_APP_VERSION` があるときだけ各行に付ける（なければ従来どおり）（#81）
- スポーツごとの予約枠の需給: admin-web の `GET /slot-supply`（本部だけ）。直近 30 日に開始する枠について、スポーツ 5 種類それぞれの公開した枠の数（`open` / `booked`）・予約しようとした生徒の数と、足りているか（生徒が枠の 60% 以上なら `insufficient`）を返す。合成シード（種 42）ではテニスとサッカーが `insufficient`（#77）
- 生徒アプリの最小の画面（`apps/student-app/`、Flutter）: アカウント登録 → スロット検索（スポーツで絞る）→ 体験予約 → 予約の確定 → 通話の開始・終了（映像はモック）または動画の送付（ファイル選択はモック）→ 添削結果 → 完了記録 → 月額契約の開始（決済はモック）（#60）
- admin-web の最小の画面（`apps/admin-web/`、Next.js）: モックのトークンでログインし、コーチは枠の公開・添削の返却・通話セッションの開始と終了、本部は予約・完了記録の一覧（API がないので「未実装」と表示）。admin-web の契約に、実装済みの `startLiveLessonSession` / `endLiveLessonSession`（student-app と同じ形）と `ErrorCode` の `invalid_booking_state` / `invalid_session_state` を追加（#59）
- 合成シード v0: `cargo run -p seed -- --seed 42`。コーチ 30・生徒 1000・12 ヶ月の履歴（上位 3 コーチに生徒の約半分、初回後 30 日以内の離脱が最多、低品質の通話 約 15%）。各集約の Postgres の保存先もこのとき実装（#57）
- バックエンドの骨格: Cargo ワークスペースと層（domain / application / infra / interface / server）、依存方向テスト、`GET /healthz`（#19）
- 契約: student-app の OpenAPI（11 操作、`x-context` 付き）と境界間イベントの AsyncAPI（BookingConfirmed / LessonCompleted / SubscriptionStarted）（#20, #21）
- DB スキーマの初版（identity / scheduling / entitlement）と expand/contract のマイグレーション基盤（#22）
- `POST /accounts`: 生徒・コーチ・本部のアカウント登録。登録時に `account.registered` の行動イベントを発火（#28）
- 認証: ロールを載せたアクセストークンの発行・検証（Gen1 はモック。形式 `mock.<ロール>.<アカウント ID>`）とロールによる認可。サーバーの起動は `cargo run -p server`（#29）
- スロット（コーチの予約枠）: `POST /slots`（コーチ。同じコーチの枠の重なりは 409）と `GET /slots`（スポーツ・コーチ・期間で絞り、カーソルでページング）。公開時に `slot.published` を発火（#30）
- 予約: `POST /bookings`（体験予約を含む。二重予約は 409 `slot_already_booked`、2 回目の体験予約は 409 `trial_already_used`）と `POST /bookings/{booking_id}/confirm`（確定でスロットが予約済みになり、境界間イベント `BookingConfirmed` を発行）。`booking.created` / `booking.confirmed` を発火（#31）
- 予約のキャンセル: `POST /bookings/{booking_id}/cancel`（確定済みならスロットが公開中に戻る。キャンセル済みは 409 `invalid_booking_state`）。キャンセル規定（開始 24 時間前以降は回数を消費）の判定を載せた境界間イベント `BookingCancelled`（`consumes_quota`）を発行し、`booking.cancelled` を発火（#32）
- 受講権の照会: `GET /entitlements/me`（生徒だけ。契約種別と、今の暦月のビデオ通話の枠 `live_lesson_quota`。月をまたげば `used` は 0 から。受講権がなければ 404）。契約に `getMyEntitlement` を追加（#33）
- 月額契約の開始: `POST /subscriptions`（生徒だけ。受講権がすでにあれば 409 `subscription_already_active`、ゴルフは 422。決済は Gen1 では扱わない）。境界間イベント `SubscriptionStarted` を発行し、`subscription.started` を発火。ビデオ通話の暦月 2 回の枠を予約の確定で使い（3 回目の確定は 409 `live_lesson_quota_exceeded`）、開始 24 時間前以降のキャンセルは枠を使ったことに、それより前のキャンセルは確定で使った枠を戻す。暦月は日本時間で区切る（#33 の UTC から変更）（#34）
- 通話セッション: `POST /live-lesson-sessions`（確定した予約の参加者（生徒・コーチ）だけ。確定前の予約は 409 `invalid_booking_state`、同じ予約の 2 回目は 409 `invalid_session_state`、`booking_id` が UUID でなければ 422）と `POST /live-lesson-sessions/{session_id}/end`（終了済みは 409 `invalid_session_state`）。通話基盤はモックで、終了時に接続品質の生の指標（約 15% は低品質の回線）を生成する。`call.started` / `call.ended`（4 属性）を発火（#37）
- 動画の送付と添削の返却: `POST /videos`（生徒だけ。保存先はモックで、返る `upload_url` に PUT する。月額は送り放題）、`GET /videos/{video_id}/review`（動画の当事者だけ。返却前は `in_progress`）、admin-web の `POST /videos/{video_id}/review`（コーチだけ。返却済みは 409 `invalid_video_state`、ゴルフの動画はワンショット決済が確定するまで 409 `review_payment_not_confirmed`）。`video.submitted` / `video_review.returned` を発火し、`video_review.returned` の必須属性に `submitted_at` を追加（#38）
- レッスンの完了記録: `POST /lessons/completions`（通話セッション・動画の当事者だけ。終了した通話セッション（`live_lesson`）または添削が返された動画（`async_lesson`）を指定する。終了前のセッションは 409 `invalid_session_state`、未返却の動画は 409 `invalid_video_state`、2 回目は 409 `lesson_already_completed`）。`is_first_lesson` はその生徒の初回だけ true。`lesson.completed` を発火し、境界間イベント `LessonCompleted` を発行する。student-app の `ErrorCode` に `invalid_video_state` を追加（#39）
- セマンティック層 `specs/analytics/kpi.yaml`（north star「初回レッスン後の 1 ヶ月継続率」を含む KPI 6 件の式。通話品質スコアは ADR 0002 Q1 の減点式）と、行動イベントのモックの配信先: `ANALYTICS_SINK=object_store` でサーバーを起動すると、行動イベントを JSON Lines で S3 API のオブジェクトストア（compose の `objectstore`）に書く（既定は従来どおりプロセス内）（#40）

### 変更
- student-app のスロットを探す画面で、各スロットの行を縦に並べ直した（日時 → スポーツ → コーチの名前 → 自己紹介文 → 行の幅いっぱいの「体験予約する」）。自己紹介文が行の幅で読めるようになり、日時は「10/28（水）3:00〜4:00」のように曜日付きの 1 行で出す（別の日に終わる枠は終わりにも日付を付ける）。API と契約の変更はない（#147）
- student-app のレッスンの評価: 点の選択肢を横並びのボタンから、「5　とてもよかった」のように点と言葉を 1 行にしたラジオボタンの縦の一覧（上から 5 → 1）に変えた。選んだものは一覧の印で分かるので、「選んだ評価: …」の表示はなくした。言葉の文言、API と契約の変更はない（#129）
- student-app のチャット: スレッドを開いたときと「新しいメッセージを確かめる」で、既読にしてから（`markMessageThreadRead`）メッセージを読み直す（`listMessages`）順に変えた。生徒の画面に出ないままコーチのメッセージが既読になることがなくなる。空白だけ・2000 文字を超える本文は送信の要求を作れない（送信ボタンも押せない）。API と契約の変更はない（#103）
- 契約: `recordLessonCompletion` の `x-context` を `catalog` から `[live-lesson, async-lesson]` に変更し、glossary の lesson を「レッスン種別」と「完了したレッスン（lesson_completion）」に分けた。行動イベント `lesson.completed` も `contexts: [live-lesson, async-lesson]` に（#55）
- `access_token` の形式が `mock-<id>` から `mock.<ロール>.<id>` に変わった。旧形式のトークンは検証に通らない（#29）
