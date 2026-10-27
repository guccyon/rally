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
