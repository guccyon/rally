import 'models.dart';
import 'student_api.dart';

/// API の失敗 → 画面の文言。契約の `ErrorCode` を日本語にするのはここだけ
/// （`Error.message` は表示用ではないので出さない）。
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
    ErrorCode.liveLessonQuotaExceeded => '今月のビデオ通話の回数（2 回）を使い切っています。',
    ErrorCode.subscriptionAlreadyActive => '月額契約はすでに始まっています。',
    ErrorCode.invalidSessionState => '通話の状態が変わっています。',
    ErrorCode.invalidVideoState => '添削がまだ返されていません。',
    ErrorCode.lessonAlreadyCompleted => 'このレッスンの完了はすでに記録されています。',
    ErrorCode.lessonAlreadyReviewed => 'このレッスンはすでに評価しています。',
    ErrorCode.unknown => 'エラーが起きました。時間をおいてやり直してください。',
  };
}
