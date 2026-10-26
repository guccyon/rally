# ADR 0006: レッスンの完了記録の置き場所と、glossary の lesson の context を見直す Question

- 状態: 提案（2026-10-26、T14 / Issue #39）。Question は未決（建設者の回答待ち）
- 関係: ADR 0002（Gen1 の Question への回答。この ADR の Question はその後継）、ADR 0004（スキーマはコンテキストごと）、`specs/glossary.yaml`（lesson）、`specs/analytics/events.yaml`（lesson.completed）、`specs/contracts/events/asyncapi.yaml`（LessonCompleted）、`specs/contracts/api/student-app/openapi.yaml`（`recordLessonCompletion`）、`.factory-config/evals/golden/0003-analytics-events.yaml`（教訓）

## Question: glossary の `lesson.context` を catalog のままにするか

glossary は lesson（コーチが生徒に提供する指導の単位）を catalog に置き、events.yaml の `lesson.completed` と student-app の `recordLessonCompletion` の `x-context` もそれに揃えて catalog にしている。しかし catalog の役割は「スポーツ、レッスン種別、コーチの提供メニュー」（context-map）で、完了の事実（誰がいつどのレッスンを受け終えたか）を持つ場所ではなく、Golden Set 0003 でも catalog / scheduling のどちらに置いても据わりが悪いと記録された。T14 では完了記録を live-lesson（通話セッション）と async-lesson（添削を返した動画）のそれぞれの集約から発行し、集約の外に lesson のコンテキストを作らなかった（下の決定）。そこで建設者に問う: **glossary の lesson を「レッスン種別」（catalog）と「完了したレッスン」（live-lesson / async-lesson が発行する事実）に分けるか、lesson.context を live-lesson / async-lesson の両方を指す形に改めるか、catalog のままにするか**。回答が出るまで glossary・events.yaml の `context`・OpenAPI の `x-context` は catalog のまま据え置き、回答に合わせて 3 つを同じ PR で直す。

## この Issue での決定（Question の回答を待たずに決めたこと）

1. **完了記録は元の集約が発行する**。domain の `LiveLessonSession::complete`（終了したセッションだけ）と `Video::complete`（添削を返した動画だけ）が `LessonCompletion` を返す。記録できるのは元の集約の当事者（生徒・コーチ）だけ。`LessonCompletion`・`LessonId`・`LessonType`・`LessonSource` は両者が共有する値で、`domain::lesson` に置く（共有カーネル。境界づけられたコンテキストではないので context-map には足さない）
2. **`is_first_lesson` はその生徒の完了記録が同期・非同期あわせて 0 件のときだけ true**。判定は `LessonCompletion` の 1 か所で、保存先（ポート `LessonCompletionRepository::record`）が同じ生徒の既存の記録を渡し、読み取りから保存までを生徒ごとに直列にする（並行な記録が両方とも初回にならないように）。同じセッション・動画の 2 回目の記録は 409 `lesson_already_completed`
3. **テーブルは元の集約と同じスキーマに置く**: `live_lesson.lesson_completions`（セッション 1 つにつき 1 行）と `async_lesson.lesson_completions`（動画 1 本につき 1 行）。lesson のスキーマは作らない。同じコンテキストの中なので、元の集約に複合外部キー（生徒・コーチが元の当事者と一致すること）を張れ、非同期は添削の行（`video_reviews`）への外部キーで「添削を返した動画だけ」も DB で守れる。生徒の初回が 1 件であることは各テーブルの部分一意索引（`lesson_completions_one_first_per_student`）までで、テーブルをまたいだ重なりは Postgres の実装が生徒ごとの advisory lock で防ぐ（ADR 0004 の 4 の `PgSlotRepository` と同じ考え方。実装は通話セッション・動画の保存先を Postgres にするときに足す）
4. 境界間イベント `LessonCompleted` のペイロードは AsyncAPI の既存の `required`（`event_id`・`occurred_at`・`student_id`・`coach_id`・`lesson_id`）のまま変えない。billing / review が要る属性（レッスンの種類など）は購読を実装するときに足す

## 選ばなかった案

- **`lesson` のスキーマに 1 つのテーブル `lesson_completions` を置く**: `is_first_lesson` の一意性を 1 つの部分一意索引で守れるが、持ち主のコンテキストがない（または catalog / 新しい lesson コンテキストになる）テーブルになり、ADR 0004 の「スキーマはコンテキストの持ち物」と食い違う。元の集約への外部キーもコンテキストをまたぐ
- **完了記録を catalog に置く（glossary どおり）**: catalog は Gen1 で何も実装しておらず、通話の終了・添削の返却を知らない。完了の判定のために catalog から live-lesson / async-lesson の中身を読むことになる
- **`is_first_lesson` を保存せず、読むたびに数える**: T15 の継続率の起点が後からの記録の追加・取り消しで動いてしまう。記録した時点の値を保存する
