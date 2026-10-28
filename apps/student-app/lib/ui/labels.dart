import '../api/models.dart';

/// 契約の値 → 画面の文言（表示の変換はここだけ）。
String sportLabel(Sport sport) => switch (sport) {
  Sport.tennis => 'テニス',
  Sport.baseball => '野球',
  Sport.soccer => 'サッカー',
  Sport.basketball => 'バスケットボール',
  Sport.swimming => '水泳',
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

/// 端末のタイムゾーンで、開始から終了までを 1 行に。例: `10/28（水）3:00〜4:00`。
/// 終了が別の日なら終了にも日付を付ける。例: `10/28（水）23:30〜10/29（木）0:30`
String formatTimeRange(DateTime start, DateTime end) {
  final from = start.toLocal();
  final to = end.toLocal();
  String day(DateTime d) => '${d.month}/${d.day}（${_weekdays[d.weekday - 1]}）';
  String time(DateTime d) => '${d.hour}:${d.minute.toString().padLeft(2, '0')}';
  final sameDay =
      from.year == to.year && from.month == to.month && from.day == to.day;
  return '${day(from)}${time(from)}〜${sameDay ? '' : day(to)}${time(to)}';
}

const _weekdays = ['月', '火', '水', '木', '金', '土', '日'];

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

/// 月額契約のプランの名前
String subscriptionPlanLabel(SubscriptionPlan plan) => switch (plan) {
  SubscriptionPlan.standard => 'スタンダード',
  SubscriptionPlan.plus => 'プラス',
};

/// プランを選ぶときに添える、ビデオ通話の暦月の上限（不変条件 1。standard 2 回・plus 4 回）。
/// 契約の前は受講権がなく上限をサーバーから受け取れないので、選ぶ前の案内のためだけにここに持つ。
/// 契約したあとの上限と残り回数は、サーバーが返す `LiveLessonQuota` を出す
int subscriptionPlanLiveLessonLimit(SubscriptionPlan plan) => switch (plan) {
  SubscriptionPlan.standard => 2,
  SubscriptionPlan.plus => 4,
};

/// プランの選択肢の 1 行（名前と回数を全角の空白で並べる）。例: `プラス　ビデオ通話 月 4 回`
String subscriptionPlanChoiceLabel(SubscriptionPlan plan) =>
    '${subscriptionPlanLabel(plan)}　ビデオ通話 月 ${subscriptionPlanLiveLessonLimit(plan)} 回';

/// 今月のビデオ通話の残り回数（上限は契約プランで決まる）。例: `月 4 回のうち残り 3 回`
String liveLessonQuotaLabel(LiveLessonQuota quota) =>
    '月 ${quota.limit} 回のうち残り ${quota.remaining} 回';
