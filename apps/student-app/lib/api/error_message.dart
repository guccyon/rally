import 'models.dart';
import 'student_api.dart';

/// API の失敗 → 画面の文言。契約の `ErrorCode` を日本語にするのはここだけ
/// （`Error.message` は表示用ではないので出さない。例外は表示名の変更の 422 だけで、[profileErrorMessageOf]）。
String errorMessageOf(Object error) {
  if (error is! StudentApiException) return '想定しないエラーが起きました。';
  final code = error.error?.code;
  if (code == null) {
    return error.statusCode == null
        ? 'サーバーにつながりませんでした。通信状態を確かめてください。'
        : 'サーバーでエラーが起きました（${error.statusCode}）。';
  }
  return switch (code) {
    ErrorCode.unauthorized => 'ログインの有効期限が切れました。登録からやり直してください。',
    ErrorCode.forbidden => 'この操作は生徒のアカウントでしか行えません。',
    ErrorCode.notFound => '対象が見つかりませんでした。',
    ErrorCode.validationFailed => '入力内容を確かめてください。',
    ErrorCode.emailAlreadyRegistered => 'このメールアドレスはすでに登録されています。',
    ErrorCode.slotAlreadyBooked => 'このスロットはすでに予約されました。別のスロットを選んでください。',
    ErrorCode.trialAlreadyUsed => '体験予約はすでに使っています。',
    ErrorCode.invalidBookingState => '予約の状態が変わっています。予約を確かめてください。',
    ErrorCode.liveLessonQuotaExceeded => '今月のビデオ通話の枠を使い切っています。',
    ErrorCode.subscriptionAlreadyActive => '月額契約はすでに始まっています。',
    ErrorCode.invalidSessionState => '通話の状態が変わっています。',
    ErrorCode.invalidVideoState => '添削がまだ返されていません。',
    ErrorCode.lessonAlreadyCompleted => 'このレッスンの完了はすでに記録されています。',
    ErrorCode.chatNotEntitled => 'チャットは月額契約を始めると使えます。',
    ErrorCode.lessonAlreadyReviewed => 'このレッスンはすでに評価しています。',
    ErrorCode.unknown => 'エラーが起きました。時間をおいてやり直してください。',
  };
}

/// 表示名の変更（`updateMyStudentProfile`）の失敗 → 画面の文言。422 `validation_failed` のときだけは、
/// どの規則に反したか（例: `表示名は 50 文字まで`）をサーバーの `Error.message` のまま出す
/// （規則を判定するのはサーバーで、アプリに書き写さない）。空なら、それ以外の失敗と同じく [errorMessageOf]。
String profileErrorMessageOf(Object error) {
  if (error
      case StudentApiException(
        error: ApiError(code: ErrorCode.validationFailed, :final message),
      )
      when message.trim().isNotEmpty) {
    return message;
  }
  return errorMessageOf(error);
}
