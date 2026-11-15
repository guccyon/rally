import 'models.dart';
import 'student_api.dart';

/// チャットを送れなかった（月額契約の前）ときの文言。画面はこれと一致したときに、月額契約へ進む入口を添える
const chatNotEntitledMessage = 'チャットは月額契約を始めると使えます。';

/// 体験を使った後の通常の予約を断られた（月額契約の前）ときの文言。画面はこれと一致したときに、月額契約へ進む入口を添える
const liveLessonNotEntitledMessage = '体験はすでに使っています。ビデオ通話のレッスンは月額契約を始めると予約できます。';

/// 動画の添削を依頼できなかった（403 `video_review_not_entitled`。体験を使い終えた月額契約の前）ときの文言。
/// 画面はこれと一致したときに、月額契約へ進む入口を添える
const videoReviewNotEntitledMessage = '体験はすでに使っています。動画の添削は月額契約を始めると使えます。';

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
    ErrorCode.chatNotEntitled => chatNotEntitledMessage,
    ErrorCode.lessonAlreadyReviewed => 'このレッスンはすでに評価しています。',
    ErrorCode.coachNotAcceptingBookings => 'このコーチは今、新しい予約を受け付けていません。',
    ErrorCode.liveLessonSessionEndedByHeadquarters =>
      'この通話は運営が終了したため、レッスンの完了を記録できません。',
    ErrorCode.slotNotStarted =>
      'この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。',
    ErrorCode.loginCodeInvalid => '確認コードが違います。メールに届いた 6 桁のコードを確かめてください。',
    ErrorCode.loginCodeExpired => '確認コードの有効期限が切れました。コードを送り直してください。',
    ErrorCode.tooManyRequests => '試した回数が多すぎます。しばらく待ってからやり直してください。',
    ErrorCode.liveLessonNotEntitled => liveLessonNotEntitledMessage,
    ErrorCode.videoReviewNotEntitled => videoReviewNotEntitledMessage,
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

/// ログアウトの前に確定待ちの予約があるかを読めなかった（`listMyBookings` の失敗）ときの文言。ログアウトはしない（#1718）
String logoutBookingsErrorMessageOf(Object error) =>
    '確定待ちの予約があるかを確かめられなかったため、ログアウトしていません。${errorMessageOf(error)}';

/// ログアウトの前に確定待ちの予約をキャンセルできなかった（`cancelBooking` の失敗）ときの文言。ログアウトはしない（#1718）
String logoutCancelErrorMessageOf(Object error) =>
    '確定待ちの予約をキャンセルできなかったため、ログアウトしていません。${errorMessageOf(error)}';
