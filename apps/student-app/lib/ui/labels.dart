import '../api/models.dart';

/// 契約の値 → 画面の文言（表示の変換はここだけ）。
String sportLabel(Sport sport) => switch (sport) {
  Sport.tennis => 'テニス',
  Sport.baseball => '野球',
  Sport.soccer => 'サッカー',
  Sport.basketball => 'バスケットボール',
};

String bookingStatusLabel(BookingStatus status) => switch (status) {
  BookingStatus.pending => '確定待ち',
  BookingStatus.confirmed => '確定済み',
  BookingStatus.cancelled => 'キャンセル済み',
};

String lessonTypeLabel(LessonType type) => switch (type) {
  LessonType.liveLesson => '同期レッスン（ビデオ通話）',
  LessonType.asyncLesson => '非同期レッスン（動画添削）',
};

/// 端末のタイムゾーンで `M/d HH:mm`
String formatDateTime(DateTime value) {
  final local = value.toLocal();
  String two(int n) => n.toString().padLeft(2, '0');
  return '${local.month}/${local.day} ${two(local.hour)}:${two(local.minute)}';
}

/// 生徒から見た送り手
String messageSenderLabel(MessageSenderRole role) => switch (role) {
  MessageSenderRole.student => '自分',
  MessageSenderRole.coach => 'コーチ',
};

/// 自分が送ったメッセージを相手が読んだか（相手から届いたものには出さない）
String? readLabel(Message message) =>
    message.senderRole != MessageSenderRole.student
    ? null
    : message.readAt == null
    ? '未読'
    : '既読';

/// レッスンの評価の点（契約では 1〜5）に添える言葉の目安。生徒ごとの評価の基準をそろえるためのもので、
/// 文言を変えるときはここだけを変える。
String ratingLabel(int rating) => switch (rating) {
  1 => '不満だった',
  2 => '少し物足りなかった',
  3 => 'ふつう',
  4 => 'よかった',
  5 => 'とてもよかった',
  _ => throw RangeError.range(rating, 1, 5, 'rating'),
};

/// 点と言葉の目安を合わせた表示。例: `4（よかった）`
String ratingWithLabel(int rating) => '$rating（${ratingLabel(rating)}）';

/// 評価の選択肢の 1 行（点と言葉の目安を全角の空白で並べる）。例: `4　よかった`
String ratingChoiceLabel(int rating) => '$rating　${ratingLabel(rating)}';

/// 通知の種類の文言。生徒に伝わるよう、いつ届く通知かで書く
String notificationKindLabel(NotificationKind kind) => switch (kind) {
  NotificationKind.bookingConfirmed => '予約が確定したとき',
  NotificationKind.videoReviewReturned => '動画の添削が返ってきたとき',
  NotificationKind.messageReceived => '担当コーチからメッセージが届いたとき',
};
