# student-app

Rally の生徒アプリの最小（Flutter stable・null safety・`flutter_lints`・状態管理は Riverpod）。Gen1 の一周シナリオの生徒側を通す。iOS / Android の実機ビルドは P0〜P1 の対象外なので、確認は `flutter analyze` と `flutter test` まで（`ios/` `android/` はまだ置かない）。

## コマンド

```sh
flutter pub get
flutter analyze   # 警告 0
flutter test      # ウィジェットテスト（API はモック）と一周テスト
flutter run --dart-define=RALLY_API_URL=http://127.0.0.1:8080   # 接続先（既定はバックエンドの既定の待ち受け）
```

## 画面

画面遷移は状態（`lib/state/student_flow.dart` の `FlowStep`）だけで決まる。API を呼ぶのは `StudentFlowNotifier` だけで、画面は状態を表示してそのメソッドを呼ぶ。

| 画面（`lib/screens/`） | 呼ぶ操作 |
|---|---|
| アカウント登録 | `registerAccount`（続けて生徒のスポーツで `searchSlots`） |
| スロットを探す（スポーツで絞る。各行は縦に 日時（例: `10/28（水）3:00〜4:00`）→ スポーツ → グループレッスンの枠（定員 2 以上）なら「グループ（定員 N 人・残り M 席）」（定員 1 の枠には出さない）→ 載っていればコーチの名前と自己紹介文 → コーチの評価の件数と平均（例: `★ 4.2（12 件）`。0 件なら「評価はまだありません」）→ 行の幅いっぱいの「体験予約する」。自己紹介文は 2 行までで、収まらなければ「続きを読む」で全文。評価は「評価を見る」でそのコーチの評価の一覧（新しい順。点・一言・送った時刻）を開く。評価の API が失敗しても検索結果と体験予約はそのままで、評価の欄にだけ読み込めなかった旨を出す）・体験予約 | `searchSlots`（続けて検索結果のコーチごとに `getCoachReviewSummary`）/ `listCoachReviews` / `createBooking`（`is_trial: true`） |
| 予約の確定待ち（確定後に通話か動画の送付を選ぶ） | `confirmBooking` |
| ビデオ通話（開始・終了のボタンだけ。映像はモック） | `startLiveLessonSession` / `endLiveLessonSession` |
| 動画を送る（ファイルの選択と本体のアップロードはモック。送り先は体験予約のコーチ、スポーツはそのスロットのもの） | `submitVideo` |
| 添削結果（未返却なら確かめ直す。AI の一次フィードバック `ai_feedback` があれば、未返却でも返却後でも「AI の一次フィードバック」として、AI が自動で作った文でコーチの添削ではない旨と一緒に、コーチの添削とは別の枠に出す。ないときは出さない） | `getVideoReview` |
| レッスンの完了 | `recordLessonCompletion`（通話なら `live_lesson`、動画なら `async_lesson`） |
| レッスンの評価（完了のあとに 1〜5 の点と一言。評価済みを返す読み取りの API はないので、この端末で送れたときと 409 `lesson_already_reviewed` が返ったときに評価済みとして入力させない。評価せずに月額契約へ進むこともできる） | `submitLessonReview` |
| 月額契約（プランをスタンダード「ビデオ通話 月 2 回」とプラス「ビデオ通話 月 4 回」から選ぶ。既定はスタンダードで、選んだプランを `plan` で送る。選ぶ前の回数の案内は `lib/ui/labels.dart` に持ち、開始後はプランと、今月のビデオ通話を応答の `live_lesson_quota` から「月 N 回のうち残り M 回」で出す。決済はモック。`payment_method_token` は固定のモック値） | `startSubscription` |
| チャット: 担当コーチとのスレッドの一覧（未読の数・最後のメッセージ。登録後はどの画面からもアプリバーの「チャット」で開き、閉じると開いた画面に戻る） | `listMessageThreads` |
| チャット: メッセージ（送った順。自分が送ったものに既読・未読。開いたときと確かめ直したときに相手のメッセージを既読にする）と送信（月額契約の前は 403 `chat_not_entitled` の文言）。入力欄のそばの「添削を依頼する」で動画を選んで（モック）送ると添削の依頼になる（一周シナリオの動画の送付と同じ `submitVideo`。送り先はスレッドの担当コーチ、スポーツは生徒のもの。普通のメッセージとして動画は送れない）。種類 `kind` が添削の依頼・返却のメッセージは、本文の吹き出しと見分けられるアイコンと見出し付きのカードで時間の順に出す（依頼は「N 回目の添削の依頼」、返却は「コーチの添削（N 回目の依頼）」とコーチの添削の本文、AI の一次フィードバックがあれば別の枠。返却の添削は動画ごとに `getVideoReview` で読み、読めなければそのカードにだけ旨を出して確かめ直したときに読み直す）。知らない種類のメッセージは隠さず `body` を本文として出す（スレッドの一覧の最後のメッセージも同じ） | `listMessages` / `markMessageThreadRead` / `sendMessage` / `submitVideo` / `getVideoReview` |
| 通知の受け取り設定（予約の確定・添削の返却・メッセージの受信の種類ごとのスイッチ。登録後はどの画面からもアプリバーの「通知の設定」で開き、閉じると開いた画面に戻る。切り替えたらその種類だけをすぐ保存し、失敗したら元に戻して文言を出す） | `getMyNotificationSettings` / `updateMyNotificationSettings` |
| 自分の予約（登録後はどの画面からもアプリバーの「自分の予約」で開き、閉じると開いた画面に戻る。キャンセル済みも含めて、日時・スポーツ・載っていればコーチの名前・状態を出す。まだ始まっていない予約（キャンセル済みを除く）には「この予約をキャンセルする」を出し、押すと確認する。月額契約の生徒が開始 24 時間前以降にキャンセルするときは「今キャンセルすると、今月のビデオ通話の回数を 1 回使います。」と添える（回数を使うかを決めるのはサーバーで、ここは同じ境界で確認の文言を出すだけ。今の時刻は `clockProvider` で、テストでは固定する）。キャンセルできたらその予約をキャンセル済みにし、失敗したら状態を変えずに文言を出す） | `listMyBookings` / `cancelBooking` |

## API

- 契約 `specs/contracts/api/student-app/openapi.yaml` のスキーマを `lib/api/models.dart` に手で写した（ジェネレーターは Java が要るので使っていない）。型名は契約のスキーマ名に揃え、項目名は lowerCamelCase にする。契約の `Error` だけは Dart の `Error` と衝突するので `ApiError`
- 操作は `lib/api/student_api.dart` の `StudentApi`（operationId と 1 対 1）、HTTP の実装は `lib/api/http_student_api.dart`。契約の `ErrorCode` → 画面の文言は `lib/api/error_message.dart` の 1 か所
- 契約の enum とのずれ・操作の有無は `test/api/contract_test.dart` が検出する。契約を変えたら `models.dart` を合わせる
