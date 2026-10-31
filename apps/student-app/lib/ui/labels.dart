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

/// 添削の状態（自分の動画の一覧の行）
String videoReviewStatusLabel(VideoReviewStatus status) => switch (status) {
  VideoReviewStatus.inProgress => '添削待ち',
  VideoReviewStatus.returned => '返却済み',
};

String lessonTypeLabel(LessonType type) => switch (type) {
  LessonType.liveLesson => '同期レッスン（ビデオ通話）',
  LessonType.asyncLesson => '非同期レッスン（動画添削）',
};

/// レッスンメニューの同期レッスン 1 回の時間（`LessonMenu.duration_minutes`）。例: `1 回 60 分`
String lessonMenuDurationLabel(int minutes) => '1 回 $minutes 分';

/// 枠に結びついたレッスンメニュー（`Slot.lesson_menu`）の名前と 1 回の時間。例: `メニュー: フォアハンドの基礎（1 回 60 分）`。
/// 時間が null（結びつけた後に非同期に直されたメニュー）なら名前だけ
String slotLessonMenuLabel(SlotLessonMenu menu) =>
    _lessonMenuLabel(menu.name, menu.durationMinutes);

/// 予約した時点のレッスンメニュー（`Booking.lesson_menu`）の名前と 1 回の時間。書き方は [slotLessonMenuLabel] と同じ。
/// 時間が null（予約した時点で非同期に直されていたメニュー）なら名前だけ
String bookingLessonMenuLabel(BookingLessonMenu menu) =>
    _lessonMenuLabel(menu.name, menu.durationMinutes);

String _lessonMenuLabel(String name, int? durationMinutes) =>
    switch (durationMinutes) {
      final minutes? => 'メニュー: $name（${lessonMenuDurationLabel(minutes)}）',
      null => 'メニュー: $name',
    };

/// 動画を送った時点で選んだレッスンメニュー（`Video.lesson_menu` / `VideoReview.lesson_menu`）。例: `メニュー: サーブの動画添削`
String videoLessonMenuLabel(VideoLessonMenu menu) => 'メニュー: ${menu.name}';

/// 評価したレッスンのレッスンメニュー（`CoachReview.lesson_menu`。admin-web の評価の一覧と同じ書き方）。例: `メニュー: フォアハンドの基礎`
String reviewLessonMenuLabel(ReviewLessonMenu menu) => 'メニュー: ${menu.name}';

/// 完了したレッスンの元が持つレッスンメニュー（`MyLessonCompletion.lesson_menu`）。例: `メニュー: フォアハンドの基礎`
String lessonCompletionMenuLabel(LessonCompletionMenu menu) =>
    'メニュー: ${menu.name}';

/// ホームの行の見出しのコーチの名前。例: `佐藤 コーチ`。名前がわからなければ `担当コーチ`
String coachNameLabel(String? displayName) =>
    displayName == null ? '担当コーチ' : '$displayName コーチ';

/// コーチで絞ったスロット検索の見出し（`searchSlots` の `coach_id`）。例: `佐藤 コーチの空き枠`
String coachSlotsHeadingLabel(String displayName) =>
    '${coachNameLabel(displayName)}の空き枠';

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

/// 枠を使わずにキャンセルできる境目の案内（端末のタイムゾーンで）。
/// 例: `10月28日 3時までなら、ビデオ通話の枠を使わずにキャンセルできます。`（分があれば `3時30分`）
String freeCancelUntilLabel(DateTime until) {
  final local = until.toLocal();
  final minute = local.minute == 0 ? '' : '${local.minute}分';
  return '${local.month}月${local.day}日 ${local.hour}時$minuteまでなら、'
      'ビデオ通話の枠を使わずにキャンセルできます。';
}

/// 今キャンセルすると枠を使う予約の案内（自分の予約の行とキャンセルの確認で同じ文）
const cancelConsumesQuotaNotice = '今キャンセルすると、今月のビデオ通話の枠を 1 回使います。';

/// 生徒から見た送り手
String messageSenderLabel(MessageSenderRole role) => switch (role) {
  MessageSenderRole.student => '自分',
  MessageSenderRole.coach => 'コーチ',
  MessageSenderRole.system => 'お知らせ',
};

/// ホームのコーチとのやり取りの 1 行（スレッドの最後のメッセージ）。添削の依頼と返却は本文ではなく出来事で書く。
/// 例: `コーチから添削が返ってきました（10/26 12:00）`・`コーチ: 体験レッスンでお待ちしています`
String messageSummaryLabel(Message message) => switch (message.kind) {
  MessageKind.videoReviewRequest =>
    '添削を依頼しました（${formatDateTime(message.sentAt)}）',
  MessageKind.videoReviewReturned =>
    'コーチから添削が返ってきました（${formatDateTime(message.sentAt)}）',
  // スタンプは body の短い説明（「スタンプ: ありがとう」）をそのまま出す（絵文字はチャットのスレッドの中だけで出す）
  MessageKind.text || MessageKind.stamp || MessageKind.other =>
    '${messageSenderLabel(message.senderRole)}: ${message.body}',
};

/// スタンプの絵柄（絵文字）。契約の `StampId` の画像はクライアントが持つ
String stampEmoji(StampId stamp) => switch (stamp) {
  StampId.thanks => '🙏',
  StampId.good => '👍',
  StampId.gotIt => '👌',
  StampId.letsGo => '💪',
  StampId.niceShot => '🎯',
  StampId.tired => '🍵',
  StampId.question => '❓',
  StampId.ok => '🙆',
};

/// スタンプに添える短い言葉（契約の `StampId` の説明の呼び名）
String stampLabel(StampId stamp) => switch (stamp) {
  StampId.thanks => 'ありがとう',
  StampId.good => 'いいね',
  StampId.gotIt => '了解',
  StampId.letsGo => 'がんばろう',
  StampId.niceShot => 'ナイスショット',
  StampId.tired => 'おつかれさま',
  StampId.question => '質問',
  StampId.ok => 'OK',
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

/// プランの選択肢の 1 行（名前と、`listSubscriptionPlans` が返したビデオ通話の暦月の上限を全角の空白で並べる）。
/// 例: `プラス　ビデオ通話 月 4 回`。上限を読めなかったときは [subscriptionPlanLabel] だけを出す
String subscriptionPlanChoiceLabel(SubscriptionPlanDetail detail) =>
    '${subscriptionPlanLabel(detail.plan)}　ビデオ通話 月 ${detail.liveLessonsPerMonth} 回';

/// 今月のビデオ通話の残り回数（上限は契約プランで決まる）。例: `月 4 回のうち残り 3 回`
String liveLessonQuotaLabel(LiveLessonQuota quota) =>
    '月 ${quota.limit} 回のうち残り ${quota.remaining} 回';

/// グループレッスンの枠（定員 2 以上）の定員と残りの席。例: `グループ（定員 4 人・残り 2 席）`。
/// 定員 1（マンツーマン）の枠には出さないので null
String? slotSeatsLabel(Slot slot) => slot.capacity < 2
    ? null
    : 'グループ（定員 ${slot.capacity} 人・残り ${slot.remainingSeats} 席）';

/// コーチの評価の件数と平均（コーチを選ぶ手がかり）。例: `★ 4.2（12 件）`。平均はサーバーが小数第 2 位まで丸めた値を、
/// 小数第 1 位に四捨五入して出す。評価が 0 件なら `評価はまだありません`
String coachReviewSummaryLabel(CoachReviewSummary summary) =>
    _ratingLabel(summary.reviewCount, summary.averageRating) ?? '評価はまだありません';

/// コーチを探す一覧の行の評価の件数と平均。書き方は [coachReviewSummaryLabel] と同じ（例: `★ 4.2（12 件）`）で、
/// 評価が 0 件なら `評価なし`
String coachListingRatingLabel(CoachListing coach) =>
    _ratingLabel(coach.reviewCount, coach.averageRating) ?? '評価なし';

/// 評価の件数と平均（平均は小数第 1 位に四捨五入）。評価が 0 件なら null
String? _ratingLabel(int count, double? average) {
  if (count == 0 || average == null) return null;
  final rounded = (average * 10).round() / 10;
  return '★ ${rounded.toStringAsFixed(1)}（$count 件）';
}

/// コーチを探す一覧の行の、そのスポーツの公開中のレッスンメニューの数。例: `レッスンメニュー 3 件`
String coachListingLessonMenuCountLabel(CoachListing coach) =>
    'レッスンメニュー ${coach.publishedLessonMenuCount} 件';

/// コーチが受けた評価 1 件の点（admin-web の評価の一覧と同じ書き方）。例: `4 点（5 点満点）`
String coachReviewRatingLabel(int rating) => '$rating 点（5 点満点）';
