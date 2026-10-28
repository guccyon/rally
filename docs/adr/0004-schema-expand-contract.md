# ADR 0004: DB スキーマの初版と expand/contract のマイグレーション

- 状態: 提案（2026-10-26、T04 / Issue #22）。T07（Issue #30）で `slots.status` とスロットの重なりの判定の置き場所を追記。T12（Issue #37）で `live_lesson.sessions` を、T13（Issue #38）で `async_lesson.videos` / `video_reviews` を、T14（Issue #39）で `live_lesson.lesson_completions` / `async_lesson.lesson_completions` を追記。D01（Issue #90）で `messaging.threads` / `messages` を、D16（Issue #134）で `notification.notification_settings` を、D18（Issue #136）で `identity.coaches.bio` を追記。D31（Issue #165）で sport の CHECK 制約の付け直し（水泳の追加）と、それを expand に書ける例外を追記。D34（Issue #168）で `async_lesson.videos` の AI の一次フィードバックの列を追記
- 関係: `docs/plans/p1-walking-skeleton.md`（段 1 の T04）、`specs/product-context.md`（不変条件）、`specs/context-map.yaml`、ADR 0002（キャンセル規定）、ADR 0003（層）、`.factory-config/manifest.yaml`（`always_human_approve` の `destructive_schema`）

## 決定

### 1. マイグレーションの置き場所と道具

- sqlx のマイグレーションを `apps/backend/crates/infra/migrations/` に置く（Postgres は infra 層の持ち物。ADR 0003）。`infra::MIGRATOR`（`sqlx::migrate!()`）がコンパイル時に埋め込むので、アプリからも同じものを当てられる
- 前進だけ（sqlx の `.up.sql` / `.down.sql` の巻き戻しは使わない）。戻すときは新しいマイグレーションで前に進めて戻す
- 適用済みのファイルは書き換えない（sqlx がチェックサムで検出して止まる）
- ローカルは `apps/backend/compose.yaml` の Postgres 16。手順は `apps/backend/README.md`

### 2. 破壊的な変更は expand → 移行 → contract の 3 段に分ける

ファイル名は `<14 桁の版>_<段>_<説明>.sql` で、段は次のどれか。1 つのマイグレーションに複数の段を混ぜない。

| 段 | ファイル名 | 書けるもの | 古い版のアプリ |
|---|---|---|---|
| expand | `..._expand_...` | テーブル・列（NULL 可か既定値つき）・索引・制約の追加。`DROP NOT NULL` / `DROP DEFAULT` などの緩和。許す値を足すだけの CHECK 制約の付け直し（同じ名前で `DROP CONSTRAINT x, ADD CONSTRAINT x CHECK (...)` を 1 つの `ALTER TABLE` に書く） | そのまま動く |
| 移行 | `..._migrate_...` | データの写し替え（`UPDATE` / `INSERT`）。アプリは新旧の両方に書く版を先に出す | そのまま動く |
| contract | `..._contract_...` | 列・テーブル・索引・制約の削除、改名、型の変更、`SET NOT NULL`、データの削除（`DELETE` / `TRUNCATE`） | 古い版が使わなくなってから当てる |

- 改名は「新しい列を expand → 両方に書いて写す → 古い列を contract」で行い、`RENAME` は使わない
- **contract は人間承認**。contract のマイグレーションを含む PR は manifest の `always_human_approve` の `destructive_schema`（列・テーブルの削除、型の縮小）に当たり、変更種別に関わらず builder の承認なしにマージしない。contract は、それを待つ expand / 移行と別の PR に分ける
- 許す値を減らす CHECK 制約の付け直しは、同じ書き方でも contract の段に分ける（古い版が書く値を拒む）。検査は値の増減までは見ないので、レビューで確かめる
- expand / 移行の段に破壊的な文（`DROP`（`DROP NOT NULL` / `DROP DEFAULT` と、同じ名前の CHECK 制約の付け直しを除く）・`RENAME`・`TRUNCATE`・`DELETE`・`ALTER ... TYPE`・`SET NOT NULL`）があれば、`crates/infra/tests/migration_conventions.rs` が `cargo test` で落とす（L1）。ファイル名の形もここで検査する

### 3. 初版のスキーマ（Gen1 のコア 3 コンテキスト）

コンテキストごとに Postgres のスキーマを分け、テーブルの持ち主を名前で示す。Gen1 は 1 つの DB で、外部キーは同じコンテキストの中と、identity（`accounts` / `coaches`）への参照だけに限る。

| スキーマ | テーブル | 内容 |
|---|---|---|
| `identity` | `accounts` | アカウント。`role` で生徒（`student`）・コーチ（`coach`）・本部（`headquarters`）を区別。生徒だけが `sport` を持つ。メールは大文字小文字を区別せず一意 |
| | `coaches` | コーチとしての情報（T05 で育てる）。`role = 'coach'` のアカウントだけを参照できる。自己紹介文 `bio`（D18 の expand で追加。既定は空文字列、`coaches_bio_length` で 400 文字まで） |
| `scheduling` | `slots` | コーチが公開する予約枠。状態（`status`）は `open` / `booked` / `closed`（T07 の expand で追加） |
| | `bookings` | 予約（体験予約は `is_trial`）。状態は `pending` / `confirmed` / `cancelled` |
| `entitlement` | `subscriptions` | 月額契約（ゴルフ以外） |
| | `entitlements` | 受講権（契約種別と利用権）。月額は元の契約と送り放題、ワンショットはゴルフと残りの添削回数 |
| | `live_lesson_quotas` | ビデオ通話の暦月の枠。受講権 × 暦月の初日で 1 行、`remaining_live_lessons` が残回数（0〜2） |
| `live_lesson` | `sessions` | 通話セッション（T12 の expand で追加）。確定した予約 1 件につき 1 つ（`sessions_booking_id_key`）。終了時に接続品質の生の指標 4 つ（`call.ended` と同じ。損失率は百分率）を揃えて持つ。予約は scheduling の持ち物なので `booking_id` に外部キーは張らない。通話品質スコアは保存しない（ADR 0002 Q1） |
| | `lesson_completions` | 同期レッスンの完了記録（T14 の expand で追加。置き場所は ADR 0006）。セッション 1 つにつき 1 行（`lesson_completions_session_id_key`）。生徒・コーチはセッションの当事者（複合外部キー `lesson_completions_session_fkey`）。`is_first_lesson` は記録した時点の値で持つ |
| `async_lesson` | `videos` | 生徒がコーチに送った動画（T13 の expand で追加）。スポーツは 6 種類（D31 の expand で水泳を追加）（ゴルフは golf-app から送る）。動画本体は保存先（S3 への直接アップロード。Gen1 はモック）に置き、ここには持たない。状態の列は持たず、添削の行の有無で決まる。AI の一次フィードバック（D34 の expand で追加。変更シナリオ cs-006）は `ai_feedback_comment` / `ai_feedback_created_at` の列で、揃って NULL（なし。AI が失敗したときと既存の動画）か揃って値を持つ（`videos_ai_feedback_shape`）。添削の行とは別に持ち、返却では書き換えない |
| | `video_reviews` | コーチが返した添削。動画 1 本につき 1 つ（`video_reviews_video_id_key`）。コメントは空でない |
| | `lesson_completions` | 非同期レッスンの完了記録（T14 の expand で追加。置き場所は ADR 0006）。添削を返した動画 1 本につき 1 行（`video_reviews` への外部キーと `lesson_completions_video_id_key`）。生徒・コーチは動画の当事者（`lesson_completions_video_fkey`） |
| `messaging` | `threads` | 生徒と担当コーチの 1 対 1 のチャットのスレッド（D01 の expand で追加。ADR 0007）。生徒とコーチの組に 1 つ（`threads_student_id_coach_id_key`）。予約の確定で開くが、予約は scheduling の持ち物なので外部キーは張らない |
| | `messages` | スレッドのメッセージ。送り手は当事者の別（`sender_role`: `student` / `coach`）で持ち、本文は空でない。既読（`read_at`）は送信の後（`messages_read_after_sent`）。送った順は `seq` |
| `notification` | `notification_settings` | 通知の受け取り設定（D16 の expand で追加）。アカウント × 通知の種類（`kind`: `booking_confirmed` / `video_review_returned` / `message_received`）で 1 行（`notification_settings_pkey`）、受け取るかは `enabled`。行のない種類は既定のオン（domain の `NotificationSettings`）として読むので、変えたことのないアカウントには行がない。通知そのものは保存しない |

列挙は Postgres の enum 型ではなく `text` と `CHECK` で持つ（値の削除・改名が enum 型だと expand/contract に分けにくい）。

### 4. DB で守る不変条件と、守らないもの

DB で守れるものは制約にする（アプリの判定が漏れても壊れたデータを入れない）。

| 不変条件 | 制約 |
|---|---|
| 1 つのスロットに有効な予約は 1 件（二重予約の防止） | `bookings_one_active_per_slot`（`status <> 'cancelled'` の部分一意索引） |
| 月額契約の生徒のビデオ通話は暦月 2 回まで（product-context の不変条件 1） | `live_lesson_quotas.remaining_live_lessons` が 0〜2。3 回目の消費は負になって入らない。枠を持てるのは月額の受講権だけ |
| 月額はゴルフ以外、ワンショットはゴルフだけ | `subscriptions.sport` の `CHECK`、`entitlements_subscription_shape` / `entitlements_one_shot_shape` |
| 月額の受講権は同じ生徒・同じスポーツの月額契約から 1 つ | `entitlements_subscription_fkey`（契約・生徒・スポーツの複合外部キー）と `subscription_id` の一意 |
| コーチの表・予約・契約の相手は正しい役割のアカウント | `(account_id, role)` への複合外部キー |
| 状態と時刻の整合（確定なら `confirmed_at`、キャンセルなら `cancelled_at`） | `bookings_status_timestamps` など |
| 動画の添削の返却は 1 回 | `video_reviews_video_id_key` |
| レッスンの完了記録はセッション・動画 1 つにつき 1 回、非同期は添削を返した動画だけ | `lesson_completions_session_id_key` / `lesson_completions_video_id_key`、`async_lesson.lesson_completions.video_id` の `video_reviews` への外部キー |
| 生徒の初回のレッスン（`is_first_lesson`）は 1 件 | テーブルごとの部分一意索引 `lesson_completions_one_first_per_student`（2 つのテーブルをまたいだ重なりは下の「制約にしないもの」） |
| 同じ生徒とコーチのチャットのスレッドは 1 つ、既読は送信の後 | `threads_student_id_coach_id_key`、`messages_read_after_sent` |
| コーチの自己紹介文は 400 文字まで（判定は domain の `CoachBio`。DB は壊れた値を入れないため） | `coaches_bio_length` |
| 動画の AI の一次フィードバックは文と時刻が揃い、文は空でなく、作ったのは送付の後（判定は domain の `AiFeedback` と `Video::record_ai_feedback`。DB は壊れた値を入れないため） | `videos_ai_feedback_shape`・`videos_ai_feedback_comment_check`・`videos_ai_feedback_after_submission` |

制約にしないもの:

- **同じコーチの枠が重ならないこと**（T07）: 判定は domain の集約 `CoachCalendar` の 1 か所に寄せ、DB の排他制約（`btree_gist` の `EXCLUDE USING gist (coach_id WITH =, tstzrange(starts_at, ends_at) WITH &&)`）は置かない。どの状態の枠が時間を占めるか（`closed` は占めない）は domain の規則で、排他制約にすると同じ規則を `WHERE` 句に写して 2 か所で持つことになり、拒否の理由（重なった枠）も domain の言葉で返しにくい。その代わり、並行な公開が互いを見落とさないよう、infra の `PgSlotRepository` はトランザクションの中でコーチごとの advisory lock（`pg_advisory_xact_lock`）を取ってから、期間に重なる枠の読み取り・判定・保存を行う。DB 以外の書き込み経路（手作業の SQL など）からの重なりは防げないことを受け入れる
- **キャンセル規定**（開始 24 時間前以降のキャンセルは回数を消費）: 判定は scheduling のアプリケーションの 1 か所に置く（ADR 0002、不変条件 3）。DB にも置くと判定が 2 か所になる。予約に「消費するか」の列も持たない（消費の実行は entitlement が `BookingCancelled` を購読して行う。T09 / T11）
- 枠の消費をいつ・どの予約で行うか、暦月をどのタイムゾーンで区切るか: T11 で決める。DB は「枠の行の残回数が 0〜2」だけを守る（T11 で決めた: ADR 0005）
- ゴルフの添削はワンショット決済が確定した動画だけ（不変条件 2）: 判定は domain の `ContractType::may_start_review` の 1 か所に置き、async-lesson は返却の前にポート `ReviewGate` で問い合わせる（T13）。決済は billing の持ち物で、Gen1 は billing のテーブルがないので DB では守らない（billing を実装するときに見直す）
- レッスンの完了記録の `is_first_lesson` の判定と、同期・非同期のテーブルをまたいだ「初回は 1 件」（T14）: 判定は domain の `LessonCompletion` の 1 か所。テーブルは元の集約のスキーマに分けた（ADR 0006）ので、DB はテーブルごとの部分一意索引までを守り、またぎは Postgres の実装が生徒ごとの advisory lock で直列にする（`PgSlotRepository` と同じ）。通話セッションが終了済みかも domain（`LiveLessonSession::complete`）で判定し、DB では守らない
- 生徒がチャットを送ってよいか（月額の送り放題。D01）: 判定は domain の `Entitlement::unlimited_video_and_chat`（規則は `ContractType`）の 1 か所で、messaging は送信の前にポート `ChatGate` で問い合わせる（ADR 0007）。送り放題なので送信の回数は保存しない
- コーチへの支払い（不変条件 4）: billing のテーブルとともに段 5 以降

### 5. 検査

- `crates/infra/tests/migration_conventions.rs`: ファイル名と段の規約（Postgres 不要。`cargo test` で常に走る）
- `crates/infra/tests/schema.rs`: 空の DB にマイグレーションを当て、テーブルの一覧と上の制約を確かめる。Postgres が要るので `#[ignore]` にし、`DATABASE_URL` を設定して `cargo test -p infra -- --include-ignored` で走らせる。CI で走らせるには CI に Postgres を足す（工場側の段 0 の作業）

## 選ばなかった案

- **sqlx の巻き戻し（`.down.sql`）**: 本番で `DROP` を含む巻き戻しを走らせること自体が破壊的変更になる。前進だけにして、戻すときも expand/contract で進める
- **スキーマを分けず public に全部置く**: 手軽だが、どのテーブルがどのコンテキストの持ち物かが名前から読めない。後でコンテキストを別 DB に切り出すときの境界も見えにくい
- **キャンセル規定を DB のトリガーで判定する**: 判定が scheduling のアプリケーションと二重になる（ADR 0002）
- **月額の通話の残回数を entitlements の列に 1 つだけ持つ**: 暦月が変わるたびに戻す処理が要り、過去の月の消費が残らない。暦月ごとの行にした
