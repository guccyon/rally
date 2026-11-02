# ADR 0004: DB スキーマの初版と expand/contract のマイグレーション

- 状態: 提案（2026-10-26、T04 / Issue #22）。T07（Issue #30）で `slots.status` とスロットの重なりの判定の置き場所を追記。T12（Issue #37）で `live_lesson.sessions` を、T13（Issue #38）で `async_lesson.videos` / `video_reviews` を、T14（Issue #39）で `live_lesson.lesson_completions` / `async_lesson.lesson_completions` を追記。D01（Issue #90）で `messaging.threads` / `messages` を、D16（Issue #134）で `notification.notification_settings` を、D18（Issue #136）で `identity.coaches.bio` を追記。D31（Issue #165）で sport の CHECK 制約の付け直し（水泳の追加）と、それを expand に書ける例外を追記。D34（Issue #168）で `async_lesson.videos` の AI の一次フィードバックの列を追記。D37（Issue #174）で `entitlement.subscriptions.plan`（契約プラン）と、`live_lesson_quotas.remaining_live_lessons` の CHECK の緩和を追記。D47（Issue #207）で `messaging.messages` の種類（`kind`）と参照（`video_id` / `review_id`）の列、`sender_role` の `system` を追記。D53（Issue #220）で 2 の緩和を個別の例外の表から形の定義に書き直し。D57（Issue #235）で 1 にサーバーの起動時のマイグレーションと、知らない版を飛ばすことを追記。D58（Issue #243）で `catalog.lesson_menus` を追記。D61（Issue #249）で `scheduling.slots.lesson_menu_id` を、D64（Issue #261）で `scheduling.bookings` の予約した時点のレッスンメニューの写しの列を追記。D108（Issue #352）で `notification.notifications`（お知らせ）を追記。D114（Issue #367）で `notification.notification_settings` / `notifications` の種類に `video_review_requested`（コーチへの添削の依頼）を追記。D126（Issue #384）で `matching.favorite_coaches`（生徒のお気に入りのコーチ）を追記。D131（Issue #389）で `review.lesson_reviews` の行と、評価へのコーチの返信の列（`reply_comment` / `replied_at`）を追記。D147（Issue #487）で `scheduling.coach_acceptance_changes`（コーチの受付を止めた・再開した記録）を追記。D148（Issue #497）で `review.lesson_reviews` の本部の対応済みの印の列（`handled_at` / `handled_by`）を追記。D187（Issue #579）で `catalog.lesson_menu_publication_changes`（本部がレッスンメニューを非公開にした・公開に戻した記録）を追記。D197（Issue #606）で `scheduling.coach_acceptance_changes.reason`（コーチの受付を止めた理由）を追記
- 関係: `docs/plans/p1-walking-skeleton.md`（段 1 の T04）、`specs/product-context.md`（不変条件）、`specs/context-map.yaml`、ADR 0002（キャンセル規定）、ADR 0003（層）、`.factory-config/manifest.yaml`（`always_human_approve` の `destructive_schema`）

## 決定

### 1. マイグレーションの置き場所と道具

- sqlx のマイグレーションを `apps/backend/crates/infra/migrations/` に置く（Postgres は infra 層の持ち物。ADR 0003）。`infra::MIGRATOR`（`sqlx::migrate!()`）がコンパイル時に埋め込むので、アプリからも同じものを当てられる
- 前進だけ（sqlx の `.up.sql` / `.down.sql` の巻き戻しは使わない）。戻すときは新しいマイグレーションで前に進めて戻す
- 適用済みのファイルは書き換えない（sqlx がチェックサムで検出して止まる）。D53 で、デプロイした環境がないうちに既存の 3 本（`20261028000005` / `000006` / `000007`）へ `-- relax:` のコメントだけを足した（SQL は同じ。チェックサムは変わるので、手元で当て済みの DB は作り直す）
- ローカルは `apps/backend/compose.yaml` の Postgres 16。手順は `apps/backend/README.md`
- サーバー（`apps/backend/crates/server` の `main`）は `DATABASE_URL` があれば、待ち受ける前に `infra::run_migrations` で `infra::MIGRATOR` を当てる。DB に当たっていて自分の知らないマイグレーション（新しい版が当てたもの）があっても止めない（sqlx の `set_ignore_missing(true)`。D57 / Issue #235 で追記）。Rollback は前の版のイメージに戻すだけで行い（factory-engine の ADR 0012）、前進だけなので新しい版が当てた expand・移行は DB に残る。expand・移行は古い版のアプリがそのまま動く変更に限っている（下の 2）ので、前の版はそれを飛ばして起動してよい。contract は古い版が使わなくなってから当てる（下の 2）ので、contract の後にそれを使っていた版へ戻すことは前提にしない。当て済みのマイグレーションの書き換え（チェックサムの不一致）は今までどおり止める

### 2. 破壊的な変更は expand → 移行 → contract の 3 段に分ける

ファイル名は `<14 桁の版>_<段>_<説明>.sql` で、段は次のどれか。1 つのマイグレーションに複数の段を混ぜない。

| 段 | ファイル名 | 書けるもの | 古い版のアプリ |
|---|---|---|---|
| expand | `..._expand_...` | テーブル・列（NULL 可か既定値つき）・索引・制約の追加。下の「緩和」 | そのまま動く |
| 移行 | `..._migrate_...` | データの写し替え（`UPDATE` / `INSERT`）。アプリは新旧の両方に書く版を先に出す | そのまま動く |
| contract | `..._contract_...` | 列・テーブル・索引・制約の削除、改名、型の変更、`SET NOT NULL`、データの削除（`DELETE` / `TRUNCATE`） | 古い版が使わなくなってから当てる |

- 改名は「新しい列を expand → 両方に書いて写す → 古い列を contract」で行い、`RENAME` は使わない
- **contract は人間承認**。contract のマイグレーションを含む PR は manifest の `always_human_approve` の `destructive_schema`（列・テーブルの削除、型の縮小）に当たり、変更種別に関わらず builder の承認なしにマージしない。contract は、それを待つ expand / 移行と別の PR に分ける
- **緩和**: データを消さず、古い版のアプリの読み書きもそのまま通る変更。expand（と移行）に書けるのは次の形だけで、`crates/infra/tests/migration_conventions.rs` が形で判定する（例外の表は持たない）
  - `DROP NOT NULL` / `DROP DEFAULT`
  - CHECK 制約の同じ名前の付け直し: `DROP CONSTRAINT x, ADD CONSTRAINT x CHECK (...)` を 1 つの `ALTER TABLE` に書く（間に制約のない瞬間を作らない）。前の定義と新しい定義がどちらも `<列> IN ('<値>', ...)` なら、検査が値を比べて、足すだけなら通し、1 つでも減らせば止める。それ以外の形（`BETWEEN` から `>=` など）は値の増減を比べられないので、文に `-- relax: 理由` があるときだけ通し、緩和かはレビューで確かめる
  - 一意・外部キーの制約の削除（`ALTER TABLE <スキーマ>.<テーブル> DROP CONSTRAINT [IF EXISTS] x`）と、一意の索引の削除（`DROP INDEX [IF EXISTS] <スキーマ>.x`）: 一意性・参照の緩和。文に `-- relax: 理由` があるときだけ通す（何が代わりに守るか・なぜ古い版も動くかを書く）。制約・索引の種類は、前のマイグレーション（同じファイルの前の文を含む）で作ったときの定義から読む。名前のない列の制約は Postgres の既定の名前（`<テーブル>_<列>_check` / `_key` / `_fkey`）で数える
- `-- relax: 理由` は、外す文の直前（前の `;` より後）か文の中に行コメントで書く。理由が空なら数えない
- 緩和でないものは、理由があっても破壊的として止める: 許す値を減らす CHECK の付け直し（contract に分ける。古い版が書く値を拒む）、`CASCADE` の付いた削除（頼っている他の制約も消す）、CHECK・主キー・一意でない索引の削除、列・テーブル・スキーマの削除
- expand / 移行の段に破壊的な文（緩和の形でない `DROP`・`RENAME`・`TRUNCATE`・`DELETE`・`ALTER ... TYPE`・`SET NOT NULL`）があれば、`crates/infra/tests/migration_conventions.rs` が `cargo test` で落とす（L1）。ファイル名の形もここで検査する

### 3. 初版のスキーマ（Gen1 のコア 3 コンテキスト）

コンテキストごとに Postgres のスキーマを分け、テーブルの持ち主を名前で示す。Gen1 は 1 つの DB で、外部キーは同じコンテキストの中と、identity（`accounts` / `coaches`）への参照だけに限る。

| スキーマ | テーブル | 内容 |
|---|---|---|
| `identity` | `accounts` | アカウント。`role` で生徒（`student`）・コーチ（`coach`）・本部（`headquarters`）を区別。生徒だけが `sport` を持つ。メールは大文字小文字を区別せず一意 |
| | `coaches` | コーチとしての情報（T05 で育てる）。`role = 'coach'` のアカウントだけを参照できる。自己紹介文 `bio`（D18 の expand で追加。既定は空文字列、`coaches_bio_length` で 400 文字まで） |
| `scheduling` | `slots` | コーチが公開する予約枠。状態（`status`）は `open` / `booked` / `closed`（T07 の expand で追加）。任意で結びついたコーチのレッスンメニュー `lesson_menu_id`（D61 の expand で追加。NULL 可で、既存の枠とメニューを指定しない枠は NULL）。メニューは catalog の持ち物なので外部キーは張らない |
| | `bookings` | 予約（体験予約は `is_trial`）。状態は `pending` / `confirmed` / `cancelled`。予約した時点で枠に結びついていたレッスンメニューの写し（D64 の expand で追加）: ID `lesson_menu_id`・名前 `lesson_menu_name`（1〜40 文字）・1 回の時間 `lesson_menu_duration_minutes`（15〜120 分の 15 分刻み。予約の時点でメニューが非同期なら NULL）。どれも NULL 可で、既存の予約とメニューなしの予約は NULL。ID と名前は揃って NULL か揃って値を持つ（`bookings_lesson_menu_shape`）。メニューは catalog の持ち物なので外部キーは張らず、後でメニューを直しても写しは書き換えない |
| | `coach_acceptance_changes` | コーチの受付（glossary の coach_acceptance）を止めた・再開した記録（D147 の expand で追加。domain の `CoachAcceptanceChange`）。止める・再開する 1 回に 1 行の追記だけの表で、受付の状態（`coach_acceptances`）と同じトランザクションで書き、書き換えない。コーチ（`identity.coaches` への外部キー）・変えた後の状態 `status`（`accepting` / `suspended`。`coach_acceptances.status` と同じ値）・操作した本部のアカウント `changed_by`（`role = 'headquarters'` のアカウントだけ: 役割つきの複合外部キー `coach_acceptance_changes_changed_by_fkey`）・変えた時刻 `changed_at`・保存した順 `seq` を持つ。この表を足す前の停止・再開は写さない（誰が操作したかが残っていない）。止めた理由 `reason`（D197 の expand で追加。domain の `CoachSuspensionReason`。止めた記録は書いた理由（書かなければ空文字列、0〜200 文字: `coach_acceptance_changes_reason_check`）、再開した記録は NULL（`coach_acceptance_changes_reason_shape`: `status = 'suspended' OR reason IS NULL`）。列を足す前の記録と古い版のアプリが書く止めた記録も NULL なので、「止めた記録は理由を持つ」は CHECK にしない） |
| `entitlement` | `subscriptions` | 月額契約（ゴルフ以外）。契約プラン `plan`（D37 の expand で追加。変更シナリオ cs-003。`standard` / `plus`、既定は `standard` で既存の契約も standard） |
| | `entitlements` | 受講権（契約種別と利用権）。月額は元の契約と送り放題、ワンショットはゴルフと残りの添削回数 |
| | `live_lesson_quotas` | ビデオ通話の暦月の枠。受講権 × 暦月の初日で 1 行、`remaining_live_lessons` が残回数（0 以上。上限は契約プランで決まる。D37 の expand で 0〜2 から緩め、既定値の 2 も外した） |
| `live_lesson` | `sessions` | 通話セッション（T12 の expand で追加）。確定した予約 1 件につき 1 つ（`sessions_booking_id_key`）。終了時に接続品質の生の指標 4 つ（`call.ended` と同じ。損失率は百分率）を揃えて持つ。予約は scheduling の持ち物なので `booking_id` に外部キーは張らない。通話品質スコアは保存しない（ADR 0002 Q1） |
| | `lesson_completions` | 同期レッスンの完了記録（T14 の expand で追加。置き場所は ADR 0006）。セッション 1 つにつき 1 行（`lesson_completions_session_id_key`）。生徒・コーチはセッションの当事者（複合外部キー `lesson_completions_session_fkey`）。`is_first_lesson` は記録した時点の値で持つ |
| `async_lesson` | `videos` | 生徒がコーチに送った動画（T13 の expand で追加）。スポーツは 6 種類（D31 の expand で水泳を追加）（ゴルフは golf-app から送る）。動画本体は保存先（S3 への直接アップロード。Gen1 はモック）に置き、ここには持たない。状態の列は持たず、添削の行の有無で決まる。AI の一次フィードバック（D34 の expand で追加。変更シナリオ cs-006）は `ai_feedback_comment` / `ai_feedback_created_at` の列で、揃って NULL（なし。AI が失敗したときと既存の動画）か揃って値を持つ（`videos_ai_feedback_shape`）。添削の行とは別に持ち、返却では書き換えない |
| | `video_reviews` | コーチが返した添削。動画 1 本につき 1 つ（`video_reviews_video_id_key`）。コメントは空でない |
| | `lesson_completions` | 非同期レッスンの完了記録（T14 の expand で追加。置き場所は ADR 0006）。添削を返した動画 1 本につき 1 行（`video_reviews` への外部キーと `lesson_completions_video_id_key`）。生徒・コーチは動画の当事者（`lesson_completions_video_fkey`） |
| `messaging` | `threads` | 生徒と担当コーチの 1 対 1 のチャットのスレッド（D01 の expand で追加。ADR 0007）。生徒とコーチの組に 1 つ（`threads_student_id_coach_id_key`）。予約の確定で開くが、予約は scheduling の持ち物なので外部キーは張らない |
| | `messages` | スレッドのメッセージ。送り手は当事者の別かシステム（`sender_role`: `student` / `coach` / `system`。`system` は D47 の expand で追加）で持ち、本文は空でない。既読（`read_at`）は送信の後（`messages_read_after_sent`）。送った順は `seq`。種類（`kind`: `text` / `video_review_request` / `video_review_return`。D47 の expand で追加、既存の行は `text`）ごとに、本文（`body`。`text` だけ。NOT NULL を外した）・動画の ID（`video_id`。添削依頼と返却）・添削の ID（`review_id`。返却だけ）を持つ（`messages_kind_shape`）。添削依頼の送り手は生徒、返却はコーチ（`messages_kind_sender`）。動画・添削は async-lesson の持ち物なので外部キーは張らない。スタンプ（`kind` の `stamp`。D54 の expand で追加）は本文・動画・添削を持たず、スタンプの ID（`stamp`。決まった 8 種類。`messages_stamp_check`）だけを持ち（`stamp` の列を持つのはスタンプだけ: `messages_stamp_only_for_stamp`）、送り手は当事者 |
| `review` | `lesson_reviews` | レッスンの評価（glossary の lesson_review）。完了したレッスン 1 件につき 1 つ（`lesson_reviews_lesson_id_key`）。評価するのは `role = 'student'` のアカウント（`lesson_reviews_student_fkey`）、評価されるのは `identity.coaches`。点は 1〜5、一言は空でない。完了したレッスンは live-lesson / async-lesson の持ち物なので `lesson_id` に外部キーは張らない。評価へのコーチの返信（D131 の expand で追加。glossary の lesson_review_reply、domain の `LessonReviewReply`）は `reply_comment`（空でなく 200 文字まで: `lesson_reviews_reply_comment_check`）と `replied_at`（評価の後: `lesson_reviews_replied_after_submission`）の列で、揃って NULL（返信なし。既存の評価も）か揃って値を持つ（`lesson_reviews_reply_shape`）。評価 1 件に 1 つで、付け直すと両方を上書きする。返信するのが評価されたコーチであることは domain の `LessonReview::write_reply` で判定し、DB では守らない。低い評価への本部の対応済みの印（D148 の expand で追加。glossary の low_rated_review_handling、domain の `LowRatedReviewHandling`）は `handled_at`（評価の後: `lesson_reviews_handled_after_submission`）と `handled_by`（`role = 'headquarters'` のアカウントだけ: 役割つきの複合外部キー `lesson_reviews_handled_by_fkey`。役割の列 `handled_by_role` は既定 `headquarters`）の列で、揃って NULL（印なし。既存の評価も）か揃って値を持つ（`lesson_reviews_handling_shape`）。評価 1 件に高々 1 つで、2 回目は書かない（`WHERE handled_at IS NULL`。最初の印を残す）。印を付けられるのが★2 以下の評価だけであることは domain の `LessonReview::mark_handled` で判定し、DB では守らない |
| `notification` | `notification_settings` | 通知の受け取り設定（D16 の expand で追加）。アカウント × 通知の種類（`kind`: `booking_confirmed` / `video_review_requested`（D114 の expand で追加）/ `video_review_returned` / `message_received`）で 1 行（`notification_settings_pkey`）、受け取るかは `enabled`。行のない種類は既定のオン（domain の `NotificationSettings`）として読むので、変えたことのないアカウントには行がない。通知そのものはここに保存しない（下の `notifications`） |
| | `notifications` | お知らせ（D108 の expand で追加。glossary の notification、domain の `Notification`）。送ったプッシュ通知 1 件につき 1 行で、受け取り設定でオフの種類も残す。受け取る人（`recipient_id`。生徒かコーチの当事者のどちらか: `notifications_recipient_is_a_party`）・プッシュ通知の種類（`kind`: `booking_confirmed` / `booking_received` / `video_review_requested`（コーチへの添削の依頼。D114 の expand で追加）/ `video_review_returned` / `message_received`。domain の `PushNotification` の変種）ごとの中身（予約の ID、依頼は動画の ID だけ、返却は動画と添削の ID、スレッドとメッセージの ID と送り手: `notifications_kind_shape`）・作った時刻・既読の時刻（作った後: `notifications_read_after_created`）・残した順 `seq` を持つ。同じプッシュ通知は 1 行（`notifications_one_per_push`。種類と予約・添削・メッセージの ID の式の一意索引。添削の依頼は動画の ID の部分一意索引 `notifications_one_per_video_review_request`）。予約・動画・添削・スレッド・メッセージは別のコンテキストの持ち物なので外部キーは張らない。本文（表示する文言）は保存しない |
| `catalog` | `lesson_menus` | コーチのレッスンメニュー（glossary の lesson「レッスン種別」。D58 の expand で追加）。コーチの提供スポーツのどれか（`identity.coach_sports` への複合外部キー `lesson_menus_coach_sport_fkey`。コーチの表への参照も兼ねる）・レッスンの種類（`lesson_type`: `live_lesson` / `async_lesson`）・名前（1〜40 文字、空白だけは不可）・説明（0〜400 文字、既定は空文字列）・同期なら 1 回の時間（`duration_minutes`。15〜120 分の 15 分刻み。非同期は NULL: `lesson_menus_duration_shape`）・公開中か（`is_published`）・作った時刻を持つ。メニューのコーチだけが直せることは domain の `LessonMenu::revise` で判定し、DB では守らない |
| | `lesson_menu_publication_changes` | 本部がレッスンメニューを非公開にした・公開に戻した記録（D187 の expand で追加。glossary の lesson_menu_publication_change、domain の `LessonMenuPublicationChange`）。非公開にする・公開に戻す 1 回に 1 行の追記だけの表で、メニューの行（`lesson_menus`）と同じトランザクションで書き、書き換えない。メニュー（`lesson_menus` への外部キー）・操作 `action`（`unpublished` / `republished`）・非公開にしたときの理由 `reason`（0〜200 文字: `lesson_menu_publication_changes_reason_check`。非公開にした記録だけが持ち、公開に戻した記録は NULL: `lesson_menu_publication_changes_reason_shape`）・操作した本部のアカウント `changed_by`（`role = 'headquarters'` のアカウントだけ: 役割つきの複合外部キー `lesson_menu_publication_changes_changed_by_fkey`）・操作した時刻 `changed_at`・保存した順 `seq` を持つ。この表を足す前の非公開・公開に戻す操作は写さない（誰が操作したかが残っていない）。コーチ自身の公開・非公開の変更は記録しない
| `matching` | `favorite_coaches` | 生徒のお気に入りのコーチ（D126 の expand で追加。glossary の favorite_coach、domain の `FavoriteCoach`）。生徒とコーチの組で 1 行（`favorite_coaches_pkey`）で、同じコーチを何度入れても 1 件（入れ直しは `ON CONFLICT DO NOTHING` で、入れた時刻 `favorited_at` は最初のまま）。生徒は `role = 'student'` のアカウントだけ（`favorite_coaches_student_fkey`。役割つきの複合外部キー）、相手は `identity.coaches` だけ（`favorite_coaches_coach_id_fkey`）。判定は domain の `FavoriteCoach::new` |

列挙は Postgres の enum 型ではなく `text` と `CHECK` で持つ（値の削除・改名が enum 型だと expand/contract に分けにくい）。

### 4. DB で守る不変条件と、守らないもの

DB で守れるものは制約にする（アプリの判定が漏れても壊れたデータを入れない）。

| 不変条件 | 制約 |
|---|---|
| 1 つのスロットに有効な予約は 1 件（二重予約の防止） | `bookings_one_active_per_slot`（`status <> 'cancelled'` の部分一意索引） |
| 月額契約の生徒のビデオ通話は契約プランで決まる暦月の上限まで（product-context の不変条件 1） | `live_lesson_quotas.remaining_live_lessons` が 0 以上。使い切った後の消費は負になって入らない。枠を持てるのは月額の受講権だけ。契約プランは `subscriptions_plan_check` の値だけ |
| 月額はゴルフ以外、ワンショットはゴルフだけ | `subscriptions.sport` の `CHECK`、`entitlements_subscription_shape` / `entitlements_one_shot_shape` |
| 月額の受講権は同じ生徒・同じスポーツの月額契約から 1 つ | `entitlements_subscription_fkey`（契約・生徒・スポーツの複合外部キー）と `subscription_id` の一意 |
| コーチの表・予約・契約の相手は正しい役割のアカウント | `(account_id, role)` への複合外部キー |
| 状態と時刻の整合（確定なら `confirmed_at`、キャンセルなら `cancelled_at`） | `bookings_status_timestamps` など |
| 動画の添削の返却は 1 回 | `video_reviews_video_id_key` |
| レッスンの完了記録はセッション・動画 1 つにつき 1 回、非同期は添削を返した動画だけ | `lesson_completions_session_id_key` / `lesson_completions_video_id_key`、`async_lesson.lesson_completions.video_id` の `video_reviews` への外部キー |
| 生徒の初回のレッスン（`is_first_lesson`）は 1 件 | テーブルごとの部分一意索引 `lesson_completions_one_first_per_student`（2 つのテーブルをまたいだ重なりは下の「制約にしないもの」） |
| 同じ生徒とコーチのチャットのスレッドは 1 つ、既読は送信の後 | `threads_student_id_coach_id_key`、`messages_read_after_sent` |
| 同じ動画の添削依頼・添削の返却はスレッドに 1 通ずつ（判定は domain の `MessageThread`。DB は境界間イベントの重複で 2 通入れないため） | `messages_one_per_video_and_kind`（`video_id` のある行の部分一意索引） |
| コーチの自己紹介文は 400 文字まで（判定は domain の `CoachBio`。DB は壊れた値を入れないため） | `coaches_bio_length` |
| 動画の AI の一次フィードバックは文と時刻が揃い、文は空でなく、作ったのは送付の後（判定は domain の `AiFeedback` と `Video::record_ai_feedback`。DB は壊れた値を入れないため） | `videos_ai_feedback_shape`・`videos_ai_feedback_comment_check`・`videos_ai_feedback_after_submission` |

制約にしないもの:

- **同じコーチの枠が重ならないこと**（T07）: 判定は domain の集約 `CoachCalendar` の 1 か所に寄せ、DB の排他制約（`btree_gist` の `EXCLUDE USING gist (coach_id WITH =, tstzrange(starts_at, ends_at) WITH &&)`）は置かない。どの状態の枠が時間を占めるか（`closed` は占めない）は domain の規則で、排他制約にすると同じ規則を `WHERE` 句に写して 2 か所で持つことになり、拒否の理由（重なった枠）も domain の言葉で返しにくい。その代わり、並行な公開が互いを見落とさないよう、infra の `PgSlotRepository` はトランザクションの中でコーチごとの advisory lock（`pg_advisory_xact_lock`）を取ってから、期間に重なる枠の読み取り・判定・保存を行う。DB 以外の書き込み経路（手作業の SQL など）からの重なりは防げないことを受け入れる
- **キャンセル規定**（開始 24 時間前以降のキャンセルは回数を消費）: 判定は scheduling のアプリケーションの 1 か所に置く（ADR 0002、不変条件 3）。DB にも置くと判定が 2 か所になる。予約に「消費するか」の列も持たない（消費の実行は entitlement が `BookingCancelled` を購読して行う。T09 / T11）
- 枠の消費をいつ・どの予約で行うか、暦月をどのタイムゾーンで区切るか: T11 で決める。DB は「枠の行の残回数が 0〜2」だけを守る（T11 で決めた: ADR 0005）
- ゴルフの添削はワンショット決済が確定した動画だけ（不変条件 2）: 判定は domain の `ContractType::may_start_review` の 1 か所に置き、async-lesson は返却の前にポート `ReviewGate` で問い合わせる（T13）。決済は billing の持ち物で、Gen1 は billing のテーブルがないので DB では守らない（billing を実装するときに見直す）
- レッスンの完了記録の `is_first_lesson` の判定と、同期・非同期のテーブルをまたいだ「初回は 1 件」（T14）: 判定は domain の `LessonCompletion` の 1 か所。テーブルは元の集約のスキーマに分けた（ADR 0006）ので、DB はテーブルごとの部分一意索引までを守り、またぎは Postgres の実装が生徒ごとの advisory lock で直列にする（`PgSlotRepository` と同じ）。通話セッションが終了済みかも domain（`LiveLessonSession::complete`）で判定し、DB では守らない
- 生徒がチャットを送ってよいか（月額の送り放題。D01）: 判定は domain の `Entitlement::unlimited_video_and_chat`（規則は `ContractType`）の 1 か所で、messaging は送信の前にポート `ChatGate` で問い合わせる（ADR 0007）。送り放題なので送信の回数は保存しない
- 契約プランごとの暦月の上限の数（D37。standard 2 回・plus 4 回）: 判定は domain の `SubscriptionPlan::live_lessons_per_month` の 1 か所で、`LiveLessonQuota` がそこから上限を受け取る。プランは `subscriptions` に、残回数は `live_lesson_quotas` にあり、上限を CHECK にするとプランの数を SQL に写して 2 か所で持つことになるので、DB は残回数が 0 以上だけを守る（上の端の 2 も外した）
- 枠に結びつけられるレッスンメニュー（D61。枠のコーチ自身の・公開中の・同期の・スポーツが枠と同じ・1 回の時間が枠の長さと同じ）: 判定は domain の `Slot::with_lesson_menu` の 1 か所で、scheduling は公開の前にポート `LessonMenuLookup` で catalog に問い合わせる。メニューは別のコンテキストの表なので外部キーも CHECK も置かない。結びつけた後にメニューを直す・非公開にすることは止めない（枠の一覧は今のメニューを添え、生徒の検索は公開中のものだけを添える）
- 予約が写すレッスンメニュー（D64。予約したスロットに結びついたメニューで、予約の時点で公開中のもの。そうでなければメニューなしの予約にし、予約は断らない）: 判定は domain の `Booking::with_lesson_menu` の 1 か所で、予約の作成は保存の前にポート `LessonMenuLookup` で catalog に問い合わせる。DB は写しの形（`bookings_lesson_menu_shape`・`bookings_lesson_menu_name_check`・`bookings_lesson_menu_duration_check`）だけを守り、スロットの `lesson_menu_id` との一致は守らない（別のコンテキストの表を読む CHECK は置けず、写しは予約の時点の値なので後で枠やメニューと食い違ってよい）
- コーチへの支払い（不変条件 4）: billing のテーブルとともに段 5 以降

### 5. 検査

- `crates/infra/tests/migration_conventions.rs`: ファイル名と段の規約（Postgres 不要。`cargo test` で常に走る）
- `crates/infra/tests/schema.rs`: 空の DB にマイグレーションを当て、テーブルの一覧と上の制約を確かめる。Postgres が要るので `#[ignore]` にし、`DATABASE_URL` を設定して `cargo test -p infra -- --include-ignored` で走らせる。CI で走らせるには CI に Postgres を足す（工場側の段 0 の作業）

## 選ばなかった案

- **sqlx の巻き戻し（`.down.sql`）**: 本番で `DROP` を含む巻き戻しを走らせること自体が破壊的変更になる。前進だけにして、戻すときも expand/contract で進める
- **スキーマを分けず public に全部置く**: 手軽だが、どのテーブルがどのコンテキストの持ち物かが名前から読めない。後でコンテキストを別 DB に切り出すときの境界も見えにくい
- **キャンセル規定を DB のトリガーで判定する**: 判定が scheduling のアプリケーションと二重になる（ADR 0002）
- **緩和を個別の例外の表（外してよい制約・索引の名前）で持つ**（D53 まで）: 緩和のたびに検査と ADR に例外を足すことになり、理由はマイグレーションの外に散る。形で判定し、理由は外す文の `-- relax:` に置くことにした
- **月額の通話の残回数を entitlements の列に 1 つだけ持つ**: 暦月が変わるたびに戻す処理が要り、過去の月の消費が残らない。暦月ごとの行にした
