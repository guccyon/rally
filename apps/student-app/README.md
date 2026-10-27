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
| スロットを探す（スポーツで絞る）・体験予約 | `searchSlots` / `createBooking`（`is_trial: true`） |
| 予約の確定待ち（確定後に通話か動画の送付を選ぶ） | `confirmBooking` |
| ビデオ通話（開始・終了のボタンだけ。映像はモック） | `startLiveLessonSession` / `endLiveLessonSession` |
| 動画を送る（ファイルの選択と本体のアップロードはモック。送り先は体験予約のコーチ、スポーツはそのスロットのもの） | `submitVideo` |
| 添削結果（未返却なら確かめ直す） | `getVideoReview` |
| レッスンの完了 | `recordLessonCompletion`（通話なら `live_lesson`、動画なら `async_lesson`） |
| レッスンの評価（完了のあとに 1〜5 の点と一言。評価済みを返す読み取りの API はないので、この端末で送れたときと 409 `lesson_already_reviewed` が返ったときに評価済みとして入力させない。評価せずに月額契約へ進むこともできる） | `submitLessonReview` |
| 月額契約（決済はモック。`payment_method_token` は固定のモック値） | `startSubscription` |
| チャット: 担当コーチとのスレッドの一覧（未読の数・最後のメッセージ。登録後はどの画面からもアプリバーの「チャット」で開き、閉じると開いた画面に戻る） | `listMessageThreads` |
| チャット: メッセージ（送った順。自分が送ったものに既読・未読。開いたときと確かめ直したときに相手のメッセージを既読にする）と送信（月額契約の前は 403 `chat_not_entitled` の文言） | `listMessages` / `markMessageThreadRead` / `sendMessage` |

## API

- 契約 `specs/contracts/api/student-app/openapi.yaml` のスキーマを `lib/api/models.dart` に手で写した（ジェネレーターは Java が要るので使っていない）。型名は契約のスキーマ名に揃え、項目名は lowerCamelCase にする。契約の `Error` だけは Dart の `Error` と衝突するので `ApiError`
- 操作は `lib/api/student_api.dart` の `StudentApi`（operationId と 1 対 1）、HTTP の実装は `lib/api/http_student_api.dart`。契約の `ErrorCode` → 画面の文言は `lib/api/error_message.dart` の 1 か所
- 契約の enum とのずれ・操作の有無は `test/api/contract_test.dart` が検出する。契約を変えたら `models.dart` を合わせる
