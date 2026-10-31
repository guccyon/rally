// student-app の契約（specs/contracts/api/student-app/openapi.yaml）の components.schemas を手で写したもの。
// 型名・項目名は契約のスキーマ名に揃える（Dart の命名規約に合わせて snake_case は lowerCamelCase に）。
// JSON との変換はこのファイルだけで行う。契約の enum の値とのずれは test/api/contract_test.dart が検出する。

/// `Sport`。student-app が扱うスポーツ（ゴルフは golf-app の範囲）。
enum Sport {
  tennis('tennis'),
  baseball('baseball'),
  soccer('soccer'),
  basketball('basketball'),
  swimming('swimming');

  const Sport(this.wire);
  final String wire;

  static Sport fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `BookingStatus`
enum BookingStatus {
  pending('pending'),
  confirmed('confirmed'),
  cancelled('cancelled');

  const BookingStatus(this.wire);
  final String wire;

  static BookingStatus fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `ContractType`。Gen1 で実装するのは subscription のみ。
enum ContractType {
  subscription('subscription'),
  oneShot('one_shot');

  const ContractType(this.wire);
  final String wire;

  static ContractType fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `SubscriptionStatus`
enum SubscriptionStatus {
  active('active'),
  cancelled('cancelled');

  const SubscriptionStatus(this.wire);
  final String wire;

  static SubscriptionStatus fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `SubscriptionPlan`（glossary の subscription_plan）。ビデオ通話の暦月の上限を決める（standard 2 回・plus 4 回）。
/// 上限の数はサーバーが返す（契約の前は `SubscriptionPlanDetail.live_lessons_per_month`、契約の後は `LiveLessonQuota.limit`）ので、ここには持たない。
enum SubscriptionPlan {
  standard('standard'),
  plus('plus');

  const SubscriptionPlan(this.wire);
  final String wire;

  static SubscriptionPlan fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `LiveLessonSessionStatus`
enum LiveLessonSessionStatus {
  inProgress('in_progress'),
  ended('ended');

  const LiveLessonSessionStatus(this.wire);
  final String wire;

  static LiveLessonSessionStatus fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `VideoStatus`
enum VideoStatus {
  awaitingUpload('awaiting_upload'),
  submitted('submitted');

  const VideoStatus(this.wire);
  final String wire;

  static VideoStatus fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `SubmitVideoRequest.content_type`
enum VideoContentType {
  mp4('video/mp4'),
  quicktime('video/quicktime');

  const VideoContentType(this.wire);
  final String wire;
}

/// `VideoReviewStatus`
enum VideoReviewStatus {
  inProgress('in_progress'),
  returned('returned');

  const VideoReviewStatus(this.wire);
  final String wire;

  static VideoReviewStatus fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `LessonType`
enum LessonType {
  liveLesson('live_lesson'),
  asyncLesson('async_lesson');

  const LessonType(this.wire);
  final String wire;

  static LessonType fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `MessageSenderRole`（スレッドのどちらの当事者が送ったか。[system] はシステムが流したもので、当事者のどちらでもない）
enum MessageSenderRole {
  student('student'),
  coach('coach'),
  system('system');

  const MessageSenderRole(this.wire);
  final String wire;

  static MessageSenderRole fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
}

/// `MessageKind`（メッセージの種類）。種類は契約で後方互換に増えるので、知らない値は [fromWire] が [other] にし、
/// そのメッセージは隠さずに `body`（種類を知らない版向けの短い説明）を本文として出す。
enum MessageKind {
  /// 当事者どうしの本文
  text('text'),

  /// 生徒が動画を添削依頼として送った（[Message.videoId]）
  videoReviewRequest('video_review_request'),

  /// コーチが動画の添削を返した（[Message.videoId]・[Message.reviewId]）
  videoReviewReturned('video_review_returned'),

  /// 当事者が本文の代わりにスタンプを送った（[Message.stampId]）
  stamp('stamp'),

  /// 契約にない（このアプリの版が知らない）種類。契約の値ではない
  other('');

  const MessageKind(this.wire);
  final String wire;

  /// 契約にない（このアプリの版が知らない）値なら [other]。
  static MessageKind fromWire(String value) => values.firstWhere(
    (e) => e != other && e.wire == value,
    orElse: () => other,
  );
}

/// `StampId`（スタンプの ID。決まった 8 種類）。画像と表示の文言はクライアントが持つ（`lib/ui/labels.dart` の `stampEmoji` / `stampLabel`）。
/// 契約で後方互換に増えうるので、知らない値は [tryFromWire] が null にする（そのメッセージは `body` の説明を出す）。
enum StampId {
  thanks('thanks'),
  good('good'),
  gotIt('got_it'),
  letsGo('lets_go'),
  niceShot('nice_shot'),
  tired('tired'),
  question('question'),
  ok('ok');

  const StampId(this.wire);
  final String wire;

  /// 契約にない（このアプリの版が知らない）値と null は null。
  static StampId? tryFromWire(String? value) =>
      values.where((e) => e.wire == value).firstOrNull;
}

/// 通知の種類（`NotificationKind`）。`NotificationSettings` と `UpdateNotificationSettingsRequest` の、受け取るかを持つ項目でもあり、
/// [wire] は項目名。契約に種類が増えたら test/api/contract_test.dart が検出するので、ここに足す。
/// お知らせ（[ApiNotification]）の種類は後方互換に増えうるので、知らない値は [tryFromWire] が null にする。
enum NotificationKind {
  bookingConfirmed('booking_confirmed'),
  videoReviewReturned('video_review_returned'),
  messageReceived('message_received');

  const NotificationKind(this.wire);
  final String wire;

  /// 契約にない（このアプリの版が知らない）値は null。
  static NotificationKind? tryFromWire(String value) =>
      values.where((e) => e.wire == value).firstOrNull;
}

/// `ErrorCode`。値は追加されうるので、知らない値は [ErrorCode.unknown] として扱う（契約の注記どおり）。
enum ErrorCode {
  unauthorized('unauthorized'),
  forbidden('forbidden'),
  notFound('not_found'),
  validationFailed('validation_failed'),
  emailAlreadyRegistered('email_already_registered'),
  slotAlreadyBooked('slot_already_booked'),
  trialAlreadyUsed('trial_already_used'),
  invalidBookingState('invalid_booking_state'),
  liveLessonQuotaExceeded('live_lesson_quota_exceeded'),
  subscriptionAlreadyActive('subscription_already_active'),
  invalidSessionState('invalid_session_state'),
  invalidVideoState('invalid_video_state'),
  lessonAlreadyCompleted('lesson_already_completed'),
  chatNotEntitled('chat_not_entitled'),
  lessonAlreadyReviewed('lesson_already_reviewed'),

  /// 契約にない（このアプリの版が知らない）値。契約の enum には含まれない。
  unknown('');

  const ErrorCode(this.wire);
  final String wire;

  static ErrorCode fromWire(String value) => values.firstWhere(
    (e) => e != unknown && e.wire == value,
    orElse: () => unknown,
  );
}

T _fromWire<T>(List<T> values, String value, String Function(T) wireOf) =>
    values.firstWhere(
      (e) => wireOf(e) == value,
      orElse: () => throw FormatException('契約にない値です: $value'),
    );

DateTime _dateTime(Object? value) => DateTime.parse(value as String);

DateTime? _dateTimeOrNull(Object? value) =>
    value == null ? null : DateTime.parse(value as String);

/// `Student`
class Student {
  const Student({
    required this.studentId,
    required this.displayName,
    required this.sport,
    required this.createdAt,
  });

  final String studentId;
  final String displayName;
  final Sport sport;
  final DateTime createdAt;

  factory Student.fromJson(Map<String, dynamic> json) => Student(
    studentId: json['student_id'] as String,
    displayName: json['display_name'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    createdAt: _dateTime(json['created_at']),
  );
}

/// `RegisterAccountRequest`
class RegisterAccountRequest {
  const RegisterAccountRequest({
    required this.email,
    required this.displayName,
    required this.sport,
  });

  final String email;
  final String displayName;
  final Sport sport;

  Map<String, dynamic> toJson() => {
    'email': email,
    'display_name': displayName,
    'sport': sport.wire,
  };
}

/// `UpdateStudentProfileRequest`（自分の表示名の変更。今は表示名だけ）。
/// 表示名の規則（1〜50 文字・空白だけは不可）はサーバーが判定し、ここでは確かめない（書いたとおりに送る）
class UpdateStudentProfileRequest {
  const UpdateStudentProfileRequest({required this.displayName});

  final String displayName;

  Map<String, dynamic> toJson() => {'display_name': displayName};
}

/// `RegisterAccountResponse`
class RegisterAccountResponse {
  const RegisterAccountResponse({
    required this.student,
    required this.accessToken,
  });

  final Student student;

  /// モックのトークン。中身は解釈しない（契約の bearerAuth）。
  final String accessToken;

  factory RegisterAccountResponse.fromJson(Map<String, dynamic> json) =>
      RegisterAccountResponse(
        student: Student.fromJson(json['student'] as Map<String, dynamic>),
        accessToken: json['access_token'] as String,
      );
}

/// `Slot`
class Slot {
  const Slot({
    required this.slotId,
    required this.coachId,
    required this.sport,
    required this.startsAt,
    required this.endsAt,
    this.capacity = 1,
    this.remainingSeats = 1,
    this.coachDisplayName,
    this.coachBio,
    this.lessonMenu,
  });

  final String slotId;
  final String coachId;
  final Sport sport;
  final DateTime startsAt;
  final DateTime endsAt;

  /// 定員（1 はマンツーマン、2 以上はグループレッスン）
  final int capacity;

  /// 残りの席（定員 − 未確定か確定の予約の数）
  final int remainingSeats;

  /// コーチの表示名（任意。コーチのアカウントが引けなければない）
  final String? coachDisplayName;

  /// コーチの自己紹介文（任意。自己紹介文を書いていないコーチはない）
  final String? coachBio;

  /// 枠に結びついたコーチのレッスンメニュー（`searchSlots` だけが載せる。メニューのない枠と、メニューが公開中でない枠は null）
  final SlotLessonMenu? lessonMenu;

  factory Slot.fromJson(Map<String, dynamic> json) => Slot(
    slotId: json['slot_id'] as String,
    coachId: json['coach_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    startsAt: _dateTime(json['starts_at']),
    endsAt: _dateTime(json['ends_at']),
    capacity: json['capacity'] as int,
    remainingSeats: json['remaining_seats'] as int,
    coachDisplayName: json['coach_display_name'] as String?,
    coachBio: json['coach_bio'] as String?,
    lessonMenu: json['lesson_menu'] == null
        ? null
        : SlotLessonMenu.fromJson(json['lesson_menu'] as Map<String, dynamic>),
  );
}

/// `SlotLessonMenu`（枠に結びついたコーチのレッスンメニューの ID・名前・1 回の時間。中身の詳しくは [LessonMenu]）
class SlotLessonMenu {
  const SlotLessonMenu({
    required this.lessonMenuId,
    required this.name,
    this.durationMinutes,
  });

  final String lessonMenuId;

  /// 名前（1〜40 文字）
  final String name;

  /// 同期レッスン 1 回の時間（分。15〜120 の 15 分刻み）。結びつけた後にメニューが非同期に直されたら null
  final int? durationMinutes;

  factory SlotLessonMenu.fromJson(Map<String, dynamic> json) => SlotLessonMenu(
    lessonMenuId: json['lesson_menu_id'] as String,
    name: json['name'] as String,
    durationMinutes: json['duration_minutes'] as int?,
  );
}

/// `SlotPage`
class SlotPage {
  const SlotPage({required this.items, this.nextCursor});

  final List<Slot> items;
  final String? nextCursor;

  factory SlotPage.fromJson(Map<String, dynamic> json) => SlotPage(
    items: (json['items'] as List<dynamic>)
        .map((e) => Slot.fromJson(e as Map<String, dynamic>))
        .toList(),
    nextCursor: json['next_cursor'] as String?,
  );
}

/// `Booking`
class Booking {
  const Booking({
    required this.bookingId,
    required this.slotId,
    required this.studentId,
    required this.coachId,
    required this.isTrial,
    required this.status,
    required this.startsAt,
    required this.createdAt,
    this.confirmedAt,
    this.cancelledAt,
    this.lessonMenu,
  });

  final String bookingId;
  final String slotId;
  final String studentId;
  final String coachId;
  final bool isTrial;
  final BookingStatus status;
  final DateTime startsAt;
  final DateTime createdAt;
  final DateTime? confirmedAt;
  final DateTime? cancelledAt;

  /// 予約した時点でスロットに結びついていたレッスンメニューの写し（予約した後にメニューが直されても変わらない）。
  /// メニューのない予約は null
  final BookingLessonMenu? lessonMenu;

  factory Booking.fromJson(Map<String, dynamic> json) => Booking(
    bookingId: json['booking_id'] as String,
    slotId: json['slot_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    isTrial: json['is_trial'] as bool,
    status: BookingStatus.fromWire(json['status'] as String),
    startsAt: _dateTime(json['starts_at']),
    createdAt: _dateTime(json['created_at']),
    confirmedAt: _dateTimeOrNull(json['confirmed_at']),
    cancelledAt: _dateTimeOrNull(json['cancelled_at']),
    lessonMenu: json['lesson_menu'] == null
        ? null
        : BookingLessonMenu.fromJson(
            json['lesson_menu'] as Map<String, dynamic>,
          ),
  );
}

/// `BookingLessonMenu`（予約した時点のコーチのレッスンメニューの ID・名前・1 回の時間。[SlotLessonMenu] と同じ形だが、今の中身ではなく写し）
class BookingLessonMenu {
  const BookingLessonMenu({
    required this.lessonMenuId,
    required this.name,
    this.durationMinutes,
  });

  final String lessonMenuId;

  /// 名前（1〜40 文字）
  final String name;

  /// 同期レッスン 1 回の時間（分。15〜120 の 15 分刻み）。予約した時点でメニューが非同期に直されていたら null
  final int? durationMinutes;

  factory BookingLessonMenu.fromJson(Map<String, dynamic> json) =>
      BookingLessonMenu(
        lessonMenuId: json['lesson_menu_id'] as String,
        name: json['name'] as String,
        durationMinutes: json['duration_minutes'] as int?,
      );
}

/// `MyBooking`（自分の予約の一覧の 1 件。予約と、予約したスロットと、今キャンセルしたら枠を使うかの見込み）
class MyBooking {
  const MyBooking({
    required this.booking,
    required this.slot,
    this.cancelConsumesQuota,
    this.freeCancelUntil,
  });

  final Booking booking;

  /// 予約したスロット（日時・スポーツ。コーチの表示名と自己紹介文は任意）
  final Slot slot;

  /// 今キャンセルしたらビデオ通話の暦月の枠を使うか。キャンセルできない予約（キャンセル済み）は null
  final bool? cancelConsumesQuota;

  /// この時刻より前のキャンセルなら枠を使わない境目（開始の 24 時間前）。境目のない予約は null
  final DateTime? freeCancelUntil;

  factory MyBooking.fromJson(Map<String, dynamic> json) => MyBooking(
    booking: Booking.fromJson(json['booking'] as Map<String, dynamic>),
    slot: Slot.fromJson(json['slot'] as Map<String, dynamic>),
    cancelConsumesQuota: json['cancel_consumes_quota'] as bool?,
    freeCancelUntil: _dateTimeOrNull(json['free_cancel_until']),
  );
}

/// `MyBookingList`
class MyBookingList {
  const MyBookingList({required this.bookings});

  /// 予約したスロットの開始時刻の新しい順（同じ開始時刻は予約の作成の新しい順）。キャンセル済みも含む
  final List<MyBooking> bookings;

  factory MyBookingList.fromJson(Map<String, dynamic> json) => MyBookingList(
    bookings: List.unmodifiable(
      (json['bookings'] as List<dynamic>).map(
        (e) => MyBooking.fromJson(e as Map<String, dynamic>),
      ),
    ),
  );
}

/// `CreateBookingRequest`
class CreateBookingRequest {
  const CreateBookingRequest({required this.slotId, required this.isTrial});

  final String slotId;
  final bool isTrial;

  Map<String, dynamic> toJson() => {'slot_id': slotId, 'is_trial': isTrial};
}

/// `LiveLessonQuota`。`limit` は月額契約のプランで決まる暦月の上限。
class LiveLessonQuota {
  const LiveLessonQuota({
    required this.month,
    required this.limit,
    required this.used,
    required this.remaining,
  });

  /// 暦月（YYYY-MM）
  final String month;
  final int limit;
  final int used;
  final int remaining;

  factory LiveLessonQuota.fromJson(Map<String, dynamic> json) =>
      LiveLessonQuota(
        month: json['month'] as String,
        limit: json['limit'] as int,
        used: json['used'] as int,
        remaining: json['remaining'] as int,
      );
}

/// `Entitlement`（glossary の entitlement）
class Entitlement {
  const Entitlement({
    required this.entitlementId,
    required this.studentId,
    required this.contractType,
    required this.sport,
    this.plan,
    this.unlimitedVideoAndChat,
    this.liveLessonQuota,
    this.remainingVideoReviews,
  });

  final String entitlementId;
  final String studentId;
  final ContractType contractType;

  /// 月額は Sport のいずれか、ワンショットは golf なので文字列のまま持つ（契約も string）。
  final String sport;

  /// 元の月額契約のプラン（月額のみ。ワンショットは null）
  final SubscriptionPlan? plan;
  final bool? unlimitedVideoAndChat;
  final LiveLessonQuota? liveLessonQuota;
  final int? remainingVideoReviews;

  factory Entitlement.fromJson(Map<String, dynamic> json) => Entitlement(
    entitlementId: json['entitlement_id'] as String,
    studentId: json['student_id'] as String,
    contractType: ContractType.fromWire(json['contract_type'] as String),
    sport: json['sport'] as String,
    plan: json['plan'] == null
        ? null
        : SubscriptionPlan.fromWire(json['plan'] as String),
    unlimitedVideoAndChat: json['unlimited_video_and_chat'] as bool?,
    liveLessonQuota: json['live_lesson_quota'] == null
        ? null
        : LiveLessonQuota.fromJson(
            json['live_lesson_quota'] as Map<String, dynamic>,
          ),
    remainingVideoReviews: json['remaining_video_reviews'] as int?,
  );
}

/// `Subscription`（glossary の subscription）
class Subscription {
  const Subscription({
    required this.subscriptionId,
    required this.studentId,
    required this.sport,
    required this.plan,
    required this.status,
    required this.startedAt,
  });

  final String subscriptionId;
  final String studentId;
  final Sport sport;
  final SubscriptionPlan plan;
  final SubscriptionStatus status;
  final DateTime startedAt;

  factory Subscription.fromJson(Map<String, dynamic> json) => Subscription(
    subscriptionId: json['subscription_id'] as String,
    studentId: json['student_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    plan: SubscriptionPlan.fromWire(json['plan'] as String),
    status: SubscriptionStatus.fromWire(json['status'] as String),
    startedAt: _dateTime(json['started_at']),
  );
}

/// `SubscriptionPlanDetail`。契約プランと、そのプランのビデオ通話の暦月の上限
class SubscriptionPlanDetail {
  /// [liveLessonsPerMonth] は契約の `minimum: 1` に合わせて 1 以上
  const SubscriptionPlanDetail({
    required this.plan,
    required this.liveLessonsPerMonth,
  }) : assert(liveLessonsPerMonth >= 1, 'live_lessons_per_month は 1 以上');

  final SubscriptionPlan plan;

  /// ビデオ通話の暦月の上限（`LiveLessonQuota.limit` と同じ値。1 以上）
  final int liveLessonsPerMonth;

  /// 必須の項目が欠けている・型が違う・`live_lessons_per_month` が 1 未満なら [FormatException]
  /// （契約に反する応答として、enum の値と同じく扱う。境界で [TypeError] を捕まえずに済むよう、ここで確かめる）
  factory SubscriptionPlanDetail.fromJson(Map<String, dynamic> json) {
    final plan = json['plan'];
    final liveLessonsPerMonth = json['live_lessons_per_month'];
    if (plan is! String) {
      throw FormatException('契約にない値です: plan=$plan');
    }
    if (liveLessonsPerMonth is! int || liveLessonsPerMonth < 1) {
      throw FormatException(
        '契約にない値です: live_lessons_per_month=$liveLessonsPerMonth',
      );
    }
    return SubscriptionPlanDetail(
      plan: SubscriptionPlan.fromWire(plan),
      liveLessonsPerMonth: liveLessonsPerMonth,
    );
  }
}

/// `SubscriptionPlanList`（`listSubscriptionPlans` の応答）
class SubscriptionPlanList {
  /// [plans] は書き換えられない写しにして持つ（`fromJson` でもコンストラクタで直接作っても、作った後に一覧を変えさせない）
  SubscriptionPlanList({required List<SubscriptionPlanDetail> plans})
    : plans = List.unmodifiable(plans);

  /// standard・plus の順
  final List<SubscriptionPlanDetail> plans;

  /// `plans` が欠けている・一覧でない・要素が object でないなら [FormatException]
  factory SubscriptionPlanList.fromJson(Map<String, dynamic> json) {
    final plans = json['plans'];
    if (plans is! List<dynamic>) {
      throw FormatException('契約にない値です: plans=$plans');
    }
    return SubscriptionPlanList(
      plans: [
        for (final e in plans)
          if (e is Map<String, dynamic>)
            SubscriptionPlanDetail.fromJson(e)
          else
            throw FormatException('契約にない値です: plans の要素=$e'),
      ],
    );
  }
}

/// `StartSubscriptionRequest`
class StartSubscriptionRequest {
  const StartSubscriptionRequest({
    required this.sport,
    this.plan,
    required this.paymentMethodToken,
  });

  final Sport sport;

  /// 契約プラン。null なら送らず、サーバーが standard にする
  final SubscriptionPlan? plan;

  /// 決済手段のトークン（Gen1 はモック。サーバーは受け取るだけで使わない）
  final String paymentMethodToken;

  Map<String, dynamic> toJson() => {
    'sport': sport.wire,
    if (plan != null) 'plan': plan!.wire,
    'payment_method_token': paymentMethodToken,
  };
}

/// `StartSubscriptionResponse`
class StartSubscriptionResponse {
  const StartSubscriptionResponse({
    required this.subscription,
    required this.entitlement,
  });

  final Subscription subscription;
  final Entitlement entitlement;

  factory StartSubscriptionResponse.fromJson(Map<String, dynamic> json) =>
      StartSubscriptionResponse(
        subscription: Subscription.fromJson(
          json['subscription'] as Map<String, dynamic>,
        ),
        entitlement: Entitlement.fromJson(
          json['entitlement'] as Map<String, dynamic>,
        ),
      );
}

/// `LiveLessonSession`
class LiveLessonSession {
  const LiveLessonSession({
    required this.sessionId,
    required this.bookingId,
    required this.studentId,
    required this.coachId,
    required this.status,
    required this.startedAt,
    this.endedAt,
    this.durationSec,
    this.roomToken,
  });

  final String sessionId;
  final String bookingId;
  final String studentId;
  final String coachId;
  final LiveLessonSessionStatus status;
  final DateTime startedAt;
  final DateTime? endedAt;
  final int? durationSec;

  /// 通話に参加するためのトークン（Gen1 はモック）。終了後は null
  final String? roomToken;

  factory LiveLessonSession.fromJson(Map<String, dynamic> json) =>
      LiveLessonSession(
        sessionId: json['session_id'] as String,
        bookingId: json['booking_id'] as String,
        studentId: json['student_id'] as String,
        coachId: json['coach_id'] as String,
        status: LiveLessonSessionStatus.fromWire(json['status'] as String),
        startedAt: _dateTime(json['started_at']),
        endedAt: _dateTimeOrNull(json['ended_at']),
        durationSec: json['duration_sec'] as int?,
        roomToken: json['room_token'] as String?,
      );
}

/// `StartLiveLessonSessionRequest`
class StartLiveLessonSessionRequest {
  const StartLiveLessonSessionRequest({required this.bookingId});

  final String bookingId;

  Map<String, dynamic> toJson() => {'booking_id': bookingId};
}

/// `Video`
class Video {
  const Video({
    required this.videoId,
    required this.studentId,
    required this.coachId,
    required this.sport,
    required this.status,
    required this.createdAt,
    this.uploadUrl,
    this.lessonMenu,
  });

  final String videoId;
  final String studentId;
  final String coachId;
  final Sport sport;
  final VideoStatus status;

  /// 動画本体の PUT 先（Gen1 はモック）
  final String? uploadUrl;
  final DateTime createdAt;

  /// 動画を送った時点で選んだレッスンメニューの写し（選ばずに送った動画は null）
  final VideoLessonMenu? lessonMenu;

  factory Video.fromJson(Map<String, dynamic> json) => Video(
    videoId: json['video_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    status: VideoStatus.fromWire(json['status'] as String),
    uploadUrl: json['upload_url'] as String?,
    createdAt: _dateTime(json['created_at']),
    lessonMenu: json['lesson_menu'] == null
        ? null
        : VideoLessonMenu.fromJson(json['lesson_menu'] as Map<String, dynamic>),
  );
}

/// `VideoLessonMenu`（動画を送った時点のコーチの非同期レッスンのレッスンメニューの ID・名前。メニューの今の中身ではなく送った時点の写し）
class VideoLessonMenu {
  const VideoLessonMenu({required this.lessonMenuId, required this.name});

  final String lessonMenuId;

  /// 名前（1〜40 文字）
  final String name;

  factory VideoLessonMenu.fromJson(Map<String, dynamic> json) =>
      VideoLessonMenu(
        lessonMenuId: json['lesson_menu_id'] as String,
        name: json['name'] as String,
      );
}

/// `SubmitVideoRequest`
class SubmitVideoRequest {
  const SubmitVideoRequest({
    required this.coachId,
    required this.sport,
    required this.contentType,
    this.note,
    this.lessonMenuId,
  });

  final String coachId;
  final Sport sport;
  final VideoContentType contentType;

  /// コーチへの相談内容（1000 文字まで）
  final String? note;

  /// 添削を頼むコーチの非同期レッスンのレッスンメニュー（任意。省けばメニューなし。送り先のコーチの・公開中の・
  /// 非同期の・[sport] と同じスポーツのメニューでなければ 422 `validation_failed`）
  final String? lessonMenuId;

  Map<String, dynamic> toJson() => {
    'coach_id': coachId,
    'sport': sport.wire,
    'content_type': contentType.wire,
    if (note != null) 'note': note,
    if (lessonMenuId != null) 'lesson_menu_id': lessonMenuId,
  };
}

/// `AiFeedback`（glossary の ai_feedback。コーチの添削の前に AI が返す一次フィードバック。コーチの添削とは別のもの）
class AiFeedback {
  const AiFeedback({required this.comment, required this.createdAt});

  final String comment;
  final DateTime createdAt;

  factory AiFeedback.fromJson(Map<String, dynamic> json) => AiFeedback(
    comment: json['comment'] as String,
    createdAt: _dateTime(json['created_at']),
  );
}

/// `VideoReview`（glossary の video_review）
class VideoReview {
  const VideoReview({
    required this.videoId,
    required this.studentId,
    required this.coachId,
    required this.status,
    this.reviewId,
    this.comment,
    this.returnedAt,
    this.aiFeedback,
    this.lessonMenu,
  });

  final String? reviewId;
  final String videoId;
  final String studentId;
  final String coachId;
  final VideoReviewStatus status;
  final String? comment;
  final DateTime? returnedAt;

  /// AI の一次フィードバック。AI が作れなかったとき（項目がない・null）は null。返却で上書きされない
  final AiFeedback? aiFeedback;

  /// 動画を送った時点で選んだレッスンメニューの写し（選ばずに送った動画は null）
  final VideoLessonMenu? lessonMenu;

  factory VideoReview.fromJson(Map<String, dynamic> json) => VideoReview(
    reviewId: json['review_id'] as String?,
    videoId: json['video_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    status: VideoReviewStatus.fromWire(json['status'] as String),
    comment: json['comment'] as String?,
    returnedAt: _dateTimeOrNull(json['returned_at']),
    aiFeedback: json['ai_feedback'] == null
        ? null
        : AiFeedback.fromJson(json['ai_feedback'] as Map<String, dynamic>),
    lessonMenu: json['lesson_menu'] == null
        ? null
        : VideoLessonMenu.fromJson(json['lesson_menu'] as Map<String, dynamic>),
  );
}

/// `MyVideo`（生徒が送った動画の一覧の 1 件。送った動画（glossary の async_lesson）と、その添削（glossary の video_review）の状態）
class MyVideo {
  const MyVideo({
    required this.videoId,
    required this.coachId,
    required this.sport,
    required this.submittedAt,
    required this.reviewStatus,
    this.coachDisplayName,
    this.returnedAt,
    this.lessonMenu,
  });

  final String videoId;

  /// 動画の送り先のコーチ
  final String coachId;

  /// 送り先のコーチの表示名。コーチのアカウントが引けないときは null
  final String? coachDisplayName;
  final Sport sport;

  /// 生徒が動画を送った時刻
  final DateTime submittedAt;
  final VideoReviewStatus reviewStatus;

  /// コーチが添削を返した時刻（添削待ちなら null）
  final DateTime? returnedAt;

  /// 動画を送った時点で選んだレッスンメニューの写し（選ばずに送った動画は null）
  final VideoLessonMenu? lessonMenu;

  factory MyVideo.fromJson(Map<String, dynamic> json) => MyVideo(
    videoId: json['video_id'] as String,
    coachId: json['coach_id'] as String,
    coachDisplayName: json['coach_display_name'] as String?,
    sport: Sport.fromWire(json['sport'] as String),
    submittedAt: _dateTime(json['submitted_at']),
    reviewStatus: VideoReviewStatus.fromWire(json['review_status'] as String),
    returnedAt: _dateTimeOrNull(json['returned_at']),
    lessonMenu: json['lesson_menu'] == null
        ? null
        : VideoLessonMenu.fromJson(json['lesson_menu'] as Map<String, dynamic>),
  );
}

/// `MyVideoList`
class MyVideoList {
  /// [videos] は書き換えられない写しにして持つ（`fromJson` でもコンストラクタで直接作っても、作った後に一覧を変えさせない）
  MyVideoList({required List<MyVideo> videos})
    : videos = List.unmodifiable(videos);

  /// 送った時刻の新しい順（同じ時刻は動画 ID の順）。添削待ちも返却済みも含む
  final List<MyVideo> videos;

  factory MyVideoList.fromJson(Map<String, dynamic> json) => MyVideoList(
    videos: [
      for (final e in json['videos'] as List<dynamic>)
        MyVideo.fromJson(e as Map<String, dynamic>),
    ],
  );
}

/// `Lesson`（完了を記録したレッスン。glossary の lesson_completion）
class Lesson {
  const Lesson({
    required this.lessonId,
    required this.studentId,
    required this.coachId,
    required this.lessonType,
    required this.isFirstLesson,
    required this.completedAt,
    this.sessionId,
    this.videoId,
  });

  final String lessonId;
  final String studentId;
  final String coachId;
  final LessonType lessonType;
  final String? sessionId;
  final String? videoId;

  /// その生徒にとって初回のレッスンか（サーバーが判定）
  final bool isFirstLesson;
  final DateTime completedAt;

  factory Lesson.fromJson(Map<String, dynamic> json) => Lesson(
    lessonId: json['lesson_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    lessonType: LessonType.fromWire(json['lesson_type'] as String),
    sessionId: json['session_id'] as String?,
    videoId: json['video_id'] as String?,
    isFirstLesson: json['is_first_lesson'] as bool,
    completedAt: _dateTime(json['completed_at']),
  );
}

/// `MyLessonCompletion`（生徒が受けたレッスンの一覧の 1 件。完了したレッスン（glossary の lesson_completion）と、
/// そのコーチの表示名・レッスンメニューの写し・評価を付けたか）
class MyLessonCompletion {
  const MyLessonCompletion({
    required this.lessonId,
    required this.coachId,
    required this.lessonType,
    required this.completedAt,
    required this.isReviewed,
    this.coachDisplayName,
    this.sessionId,
    this.videoId,
    this.lessonMenu,
  });

  /// 完了したレッスン（`Lesson.lessonId`。評価の `submitLessonReview` に渡す）
  final String lessonId;

  /// レッスンのコーチ
  final String coachId;

  /// コーチの表示名。コーチのアカウントが引けないときは null
  final String? coachDisplayName;
  final LessonType lessonType;

  /// 同期レッスンの通話セッション（非同期なら null）
  final String? sessionId;

  /// 非同期レッスンの動画（同期なら null）
  final String? videoId;

  /// 完了を記録した時刻
  final DateTime completedAt;

  /// レッスンの元（同期は予約、非同期は動画）が持つレッスンメニューの写し。元にメニューがなければ null
  final LessonCompletionMenu? lessonMenu;

  /// 本人がこの完了したレッスンに評価を付けたか
  final bool isReviewed;

  factory MyLessonCompletion.fromJson(Map<String, dynamic> json) =>
      MyLessonCompletion(
        lessonId: json['lesson_id'] as String,
        coachId: json['coach_id'] as String,
        coachDisplayName: json['coach_display_name'] as String?,
        lessonType: LessonType.fromWire(json['lesson_type'] as String),
        sessionId: json['session_id'] as String?,
        videoId: json['video_id'] as String?,
        completedAt: _dateTime(json['completed_at']),
        lessonMenu: json['lesson_menu'] == null
            ? null
            : LessonCompletionMenu.fromJson(
                json['lesson_menu'] as Map<String, dynamic>,
              ),
        isReviewed: json['is_reviewed'] as bool,
      );
}

/// `LessonCompletionMenu`（完了したレッスンのレッスンメニューの ID・名前。メニューの今の中身ではなくレッスンの元が持つ写し）
class LessonCompletionMenu {
  const LessonCompletionMenu({required this.lessonMenuId, required this.name});

  final String lessonMenuId;

  /// 名前（1〜40 文字）
  final String name;

  factory LessonCompletionMenu.fromJson(Map<String, dynamic> json) =>
      LessonCompletionMenu(
        lessonMenuId: json['lesson_menu_id'] as String,
        name: json['name'] as String,
      );
}

/// `MyLessonCompletionList`
class MyLessonCompletionList {
  /// [lessonCompletions] は書き換えられない写しにして持つ
  MyLessonCompletionList({required List<MyLessonCompletion> lessonCompletions})
    : lessonCompletions = List.unmodifiable(lessonCompletions);

  /// 完了した時刻の新しい順（同じ時刻は `lesson_id` の順）。通話も動画添削も含む
  final List<MyLessonCompletion> lessonCompletions;

  factory MyLessonCompletionList.fromJson(Map<String, dynamic> json) =>
      MyLessonCompletionList(
        lessonCompletions: [
          for (final e in json['lesson_completions'] as List<dynamic>)
            MyLessonCompletion.fromJson(e as Map<String, dynamic>),
        ],
      );
}

/// `RecordLessonCompletionRequest`（`LiveLessonCompletion` / `AsyncLessonCompletion` の oneOf）
sealed class RecordLessonCompletionRequest {
  const RecordLessonCompletionRequest();

  Map<String, dynamic> toJson();
}

/// `LiveLessonCompletion`（終了した通話セッションを指定）
class LiveLessonCompletion extends RecordLessonCompletionRequest {
  const LiveLessonCompletion({required this.sessionId});

  final String sessionId;

  @override
  Map<String, dynamic> toJson() => {
    'lesson_type': LessonType.liveLesson.wire,
    'session_id': sessionId,
  };
}

/// `AsyncLessonCompletion`（添削が返却された動画を指定）
class AsyncLessonCompletion extends RecordLessonCompletionRequest {
  const AsyncLessonCompletion({required this.videoId});

  final String videoId;

  @override
  Map<String, dynamic> toJson() => {
    'lesson_type': LessonType.asyncLesson.wire,
    'video_id': videoId,
  };
}

/// `LessonReview`（glossary の lesson_review。完了したレッスン 1 件につき 1 回だけ付ける評価）
class LessonReview {
  const LessonReview({
    required this.reviewId,
    required this.lessonId,
    required this.studentId,
    required this.coachId,
    required this.rating,
    required this.comment,
    required this.submittedAt,
  });

  final String reviewId;

  /// 評価した完了したレッスン（`Lesson.lessonId`）
  final String lessonId;
  final String studentId;

  /// 評価されたコーチ（そのレッスンのコーチ）
  final String coachId;

  /// 1（低い）〜5（高い）
  final int rating;

  /// 一言
  final String comment;
  final DateTime submittedAt;

  factory LessonReview.fromJson(Map<String, dynamic> json) => LessonReview(
    reviewId: json['review_id'] as String,
    lessonId: json['lesson_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    rating: json['rating'] as int,
    comment: json['comment'] as String,
    submittedAt: _dateTime(json['submitted_at']),
  );
}

/// `SubmitLessonReviewRequest`
class SubmitLessonReviewRequest {
  const SubmitLessonReviewRequest({
    required this.lessonId,
    required this.rating,
    required this.comment,
  });

  /// 完了したレッスン（`Lesson.lessonId`）
  final String lessonId;

  /// 1（低い）〜5（高い）
  final int rating;

  /// 一言（200 文字まで。空白だけは不可）
  final String comment;

  Map<String, dynamic> toJson() => {
    'lesson_id': lessonId,
    'rating': rating,
    'comment': comment,
  };
}

/// `CoachReviewSummary`（コーチ 1 人の lesson_review の件数と平均。数え方と丸めは契約の `getCoachReviewSummary`）
class CoachReviewSummary {
  const CoachReviewSummary({
    required this.coachId,
    required this.reviewCount,
    required this.averageRating,
  });

  final String coachId;

  /// 評価の件数（これまでのすべて）
  final int reviewCount;

  /// 点（1〜5）の平均。小数第 2 位まで（サーバーが丸める）。評価が 0 件なら null
  final double? averageRating;

  factory CoachReviewSummary.fromJson(Map<String, dynamic> json) =>
      CoachReviewSummary(
        coachId: json['coach_id'] as String,
        reviewCount: json['review_count'] as int,
        averageRating: (json['average_rating'] as num?)?.toDouble(),
      );
}

/// `CoachListing`（コーチを探す一覧 `listCoachesForStudent` の 1 件。表示名・評価の件数と平均・そのスポーツの公開中のメニューの数）
class CoachListing {
  const CoachListing({
    required this.coachId,
    required this.displayName,
    required this.sport,
    required this.reviewCount,
    required this.averageRating,
    required this.publishedLessonMenuCount,
    this.isFavorite = false,
  });

  final String coachId;

  /// コーチの表示名
  final String displayName;

  /// 探したスポーツ
  final Sport sport;

  /// 評価の件数（[CoachReviewSummary.reviewCount] と同じ。スポーツで分けない）
  final int reviewCount;

  /// 点（1〜5）の平均（[CoachReviewSummary.averageRating] と同じ。スポーツで分けない）。評価が 0 件なら null
  final double? averageRating;

  /// [sport] の公開中のレッスンメニューの数
  final int publishedLessonMenuCount;

  /// 探した生徒のお気に入り（`addFavoriteCoach`）に入っているか。項目のない応答（足す前のサーバー）は false で読む
  final bool isFavorite;

  factory CoachListing.fromJson(Map<String, dynamic> json) => CoachListing(
    coachId: json['coach_id'] as String,
    displayName: json['display_name'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    reviewCount: json['review_count'] as int,
    averageRating: (json['average_rating'] as num?)?.toDouble(),
    publishedLessonMenuCount: json['published_lesson_menu_count'] as int,
    isFavorite: json['is_favorite'] as bool? ?? false,
  );
}

/// `CoachListingList`。一覧をコンストラクタで写して持つので const では作らない。
class CoachListingList {
  // 渡した元のリストを後で変えても値が変わらないよう、読み取り専用の写しを持つ
  CoachListingList({required List<CoachListing> coaches})
    : coaches = List.unmodifiable(coaches);

  /// 評価の平均の高い順（評価のないコーチは最後）→ 表示名 → `coach_id` の順（サーバーが並べる）。読み取り専用
  final List<CoachListing> coaches;

  factory CoachListingList.fromJson(Map<String, dynamic> json) =>
      CoachListingList(
        coaches: [
          for (final e in json['coaches'] as List<dynamic>)
            CoachListing.fromJson(e as Map<String, dynamic>),
        ],
      );
}

/// `CoachProfile`（コーチの紹介 `getCoachProfile`。glossary の coach の、生徒に見せる表示名・自己紹介文・提供スポーツ）。
/// 一覧をコンストラクタで写して持つので const では作らない。
class CoachProfile {
  // 渡した元のリストを後で変えても値が変わらないよう、読み取り専用の写しを持つ
  CoachProfile({
    required this.coachId,
    required this.displayName,
    required this.bio,
    required List<Sport> sports,
    this.isFavorite = false,
  }) : sports = List.unmodifiable(sports);

  final String coachId;

  /// コーチの表示名
  final String displayName;

  /// 自己紹介文（書いたとおり）。書いていないコーチは null
  final String? bio;

  /// 提供するスポーツ（`Sport` の enum の順。ゴルフは載らない）。読み取り専用
  final List<Sport> sports;

  /// 見た生徒のお気に入り（`addFavoriteCoach`）に入っているか。項目のない応答（足す前のサーバー）は false で読む
  final bool isFavorite;

  factory CoachProfile.fromJson(Map<String, dynamic> json) => CoachProfile(
    coachId: json['coach_id'] as String,
    displayName: json['display_name'] as String,
    bio: json['bio'] as String?,
    sports: [
      for (final e in json['sports'] as List<dynamic>)
        Sport.fromWire(e as String),
    ],
    isFavorite: json['is_favorite'] as bool? ?? false,
  );
}

/// `CoachReview`（コーチが受けた lesson_review 1 件。評価した生徒は載らない）
class CoachReview {
  const CoachReview({
    required this.reviewId,
    required this.lessonId,
    required this.rating,
    required this.comment,
    required this.submittedAt,
    this.lessonMenu,
  });

  final String reviewId;

  /// 評価された完了したレッスン
  final String lessonId;

  /// 1（低い）〜5（高い）
  final int rating;

  /// 一言
  final String comment;

  /// 評価を送った時刻
  final DateTime submittedAt;

  /// 評価した完了したレッスンの元（予約・動画）が持つレッスンメニューの写し。元にメニューがなければ null
  final ReviewLessonMenu? lessonMenu;

  factory CoachReview.fromJson(Map<String, dynamic> json) => CoachReview(
    reviewId: json['review_id'] as String,
    lessonId: json['lesson_id'] as String,
    rating: json['rating'] as int,
    comment: json['comment'] as String,
    submittedAt: _dateTime(json['submitted_at']),
    lessonMenu: json['lesson_menu'] == null
        ? null
        : ReviewLessonMenu.fromJson(
            json['lesson_menu'] as Map<String, dynamic>,
          ),
  );
}

/// `ReviewLessonMenu`（評価した完了したレッスンのレッスンメニューの ID・名前。メニューの今の中身ではなくレッスンの元が持つ写し）
class ReviewLessonMenu {
  const ReviewLessonMenu({required this.lessonMenuId, required this.name});

  final String lessonMenuId;

  /// 名前（1〜40 文字）
  final String name;

  factory ReviewLessonMenu.fromJson(Map<String, dynamic> json) =>
      ReviewLessonMenu(
        lessonMenuId: json['lesson_menu_id'] as String,
        name: json['name'] as String,
      );
}

/// `CoachReviewList`
class CoachReviewList {
  const CoachReviewList({required this.coachId, required this.reviews});

  final String coachId;

  /// 送った時刻の新しい順（同じ時刻は `review_id` の順）
  final List<CoachReview> reviews;

  factory CoachReviewList.fromJson(Map<String, dynamic> json) =>
      CoachReviewList(
        coachId: json['coach_id'] as String,
        reviews: List.unmodifiable(
          (json['reviews'] as List<dynamic>).map(
            (e) => CoachReview.fromJson(e as Map<String, dynamic>),
          ),
        ),
      );
}

/// `LessonMenu`（glossary の lesson「レッスン種別」。スポーツごとにコーチが提供するメニューで、同期と非同期の別を持つ。
/// 完了したレッスンの [Lesson] とは別のもの）
class LessonMenu {
  const LessonMenu({
    required this.lessonMenuId,
    required this.coachId,
    required this.sport,
    required this.lessonType,
    required this.name,
    required this.description,
    required this.isPublished,
    required this.createdAt,
    this.durationMinutes,
  });

  final String lessonMenuId;

  /// メニューを提供するコーチ
  final String coachId;
  final Sport sport;
  final LessonType lessonType;

  /// 名前（1〜40 文字）
  final String name;

  /// 説明（0〜400 文字。書いていなければ空文字列）
  final String description;

  /// 同期レッスン 1 回の時間（分。15〜120 の 15 分刻み）。非同期レッスンなら null
  final int? durationMinutes;

  /// 公開中か（`listCoachLessonMenus` が返すのは公開中のものだけ）
  final bool isPublished;

  /// 作った時刻
  final DateTime createdAt;

  factory LessonMenu.fromJson(Map<String, dynamic> json) => LessonMenu(
    lessonMenuId: json['lesson_menu_id'] as String,
    coachId: json['coach_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    lessonType: LessonType.fromWire(json['lesson_type'] as String),
    name: json['name'] as String,
    description: json['description'] as String,
    durationMinutes: json['duration_minutes'] as int?,
    isPublished: json['is_published'] as bool,
    createdAt: _dateTime(json['created_at']),
  );
}

/// `LessonMenuList`
class LessonMenuList {
  const LessonMenuList({required this.coachId, required this.lessonMenus});

  final String coachId;

  /// 作った時刻の古い順（同じ時刻は `lesson_menu_id` の順）
  final List<LessonMenu> lessonMenus;

  factory LessonMenuList.fromJson(Map<String, dynamic> json) => LessonMenuList(
    coachId: json['coach_id'] as String,
    lessonMenus: List.unmodifiable(
      (json['lesson_menus'] as List<dynamic>).map(
        (e) => LessonMenu.fromJson(e as Map<String, dynamic>),
      ),
    ),
  );
}

/// `Message`（glossary の message。種類・送信時刻と既読を持つ）
class Message {
  const Message({
    required this.messageId,
    required this.threadId,
    required this.senderId,
    required this.senderRole,
    required this.body,
    required this.sentAt,
    this.kind = MessageKind.text,
    this.videoId,
    this.reviewId,
    this.stampId,
    this.readAt,
  });

  final String messageId;
  final String threadId;

  /// 送ったアカウント（スレッドの生徒か担当コーチ）。システムが流したものは nil UUID
  final String senderId;
  final MessageSenderRole senderRole;

  /// 種類。中身は種類ごとに [body]（本文）か [videoId]・[reviewId] か [stampId]
  final MessageKind kind;

  /// [MessageKind.text] なら本文。それ以外の種類では、種類を知らない版向けの短い説明（[MessageKind.other] ではこれを本文として出す）
  final String body;

  /// 添削を依頼した・返した動画。[MessageKind.videoReviewRequest]・[MessageKind.videoReviewReturned] のときだけある
  final String? videoId;

  /// 返した添削。[MessageKind.videoReviewReturned] のときだけある
  final String? reviewId;

  /// 送ったスタンプ。[MessageKind.stamp] のときだけある（このアプリの版が知らない ID なら null）
  final StampId? stampId;
  final DateTime sentAt;

  /// 受け取った側が読んだ時刻（既読）。読むまでは null
  final DateTime? readAt;

  /// 知らない種類（`kind`）は [MessageKind.other] として読む（読み飛ばさない）。
  factory Message.fromJson(Map<String, dynamic> json) => Message(
    messageId: json['message_id'] as String,
    threadId: json['thread_id'] as String,
    senderId: json['sender_id'] as String,
    senderRole: MessageSenderRole.fromWire(json['sender_role'] as String),
    kind: MessageKind.fromWire(json['kind'] as String),
    body: json['body'] as String,
    videoId: json['video_id'] as String?,
    reviewId: json['review_id'] as String?,
    stampId: StampId.tryFromWire(json['stamp_id'] as String?),
    sentAt: _dateTime(json['sent_at']),
    readAt: _dateTimeOrNull(json['read_at']),
  );
}

/// `MessageThread`（glossary の message_thread。生徒と担当コーチの組に 1 つ）
class MessageThread {
  const MessageThread({
    required this.threadId,
    required this.studentId,
    required this.coachId,
    required this.openedAt,
    required this.unreadCount,
    this.lastMessage,
  });

  final String threadId;
  final String studentId;
  final String coachId;

  /// スレッドを開いた時刻（その生徒とコーチの最初の予約が確定した時刻）
  final DateTime openedAt;

  /// 最後のメッセージ。まだなければ null（知らない種類も [MessageKind.other] として持つ）
  final Message? lastMessage;

  /// 呼んだアカウントにとっての未読（相手から届いてまだ読んでいない）の数
  final int unreadCount;

  factory MessageThread.fromJson(Map<String, dynamic> json) => MessageThread(
    threadId: json['thread_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    openedAt: _dateTime(json['opened_at']),
    lastMessage: json['last_message'] == null
        ? null
        : Message.fromJson(json['last_message'] as Map<String, dynamic>),
    unreadCount: json['unread_count'] as int,
  );
}

/// `MessageThreadList`
class MessageThreadList {
  const MessageThreadList({required this.threads});

  /// 最後の出来事の新しい順
  final List<MessageThread> threads;

  factory MessageThreadList.fromJson(Map<String, dynamic> json) =>
      MessageThreadList(
        threads: List.unmodifiable(
          (json['threads'] as List<dynamic>).map(
            (e) => MessageThread.fromJson(e as Map<String, dynamic>),
          ),
        ),
      );
}

/// `MessageList`
class MessageList {
  const MessageList({required this.threadId, required this.messages});

  final String threadId;

  /// 送った順。知らない種類のメッセージも [MessageKind.other] として含める
  final List<Message> messages;

  factory MessageList.fromJson(Map<String, dynamic> json) => MessageList(
    threadId: json['thread_id'] as String,
    messages: List.unmodifiable(
      (json['messages'] as List<dynamic>).map(
        (e) => Message.fromJson(e as Map<String, dynamic>),
      ),
    ),
  );
}

/// `SendMessageRequest`（本文かスタンプのどちらか 1 つ）
class SendMessageRequest {
  /// 本文を送る。本文が空白だけ、または [maxBodyLength] 文字を超えるときは [ArgumentError]
  factory SendMessageRequest({required String body}) {
    if (!isValidBody(body)) {
      throw ArgumentError.value(
        body,
        'body',
        '空白だけの本文と、$maxBodyLength 文字を超える本文は送れません',
      );
    }
    return SendMessageRequest._(body, null);
  }

  /// 本文の代わりにスタンプを送る
  const SendMessageRequest.stamp(StampId stampId) : this._(null, stampId);

  /// 契約の `SendMessageRequest.body` に収まるか（空白だけでなく、[maxBodyLength] 文字以内）。
  /// 契約の maxLength（JSON Schema）は Unicode のコードポイントで数える。画面の送信可否もこれを使う
  static bool isValidBody(String body) =>
      body.trim().isNotEmpty && body.runes.length <= maxBodyLength;

  const SendMessageRequest._(this.body, this.stampId);

  /// 契約の `SendMessageRequest.body` の maxLength
  static const maxBodyLength = 2000;

  /// 本文（2000 文字まで。空白だけは不可）。スタンプを送るときは null
  final String? body;

  /// スタンプ。本文を送るときは null
  final StampId? stampId;

  Map<String, dynamic> toJson() => {'body': ?body, 'stamp_id': ?stampId?.wire};
}

/// `NotificationSettings`（glossary の notification_settings。種類ごとに受け取るか。既定はすべて true）
class NotificationSettings {
  const NotificationSettings({
    required this.studentId,
    required this.bookingConfirmed,
    required this.videoReviewReturned,
    required this.messageReceived,
  });

  final String studentId;

  /// 予約が確定したときの通知を受け取るか
  final bool bookingConfirmed;

  /// 動画の添削が返却されたときの通知を受け取るか
  final bool videoReviewReturned;

  /// 担当コーチからチャットのメッセージが届いたときの通知を受け取るか
  final bool messageReceived;

  /// [kind] の通知を受け取るか
  bool receives(NotificationKind kind) => switch (kind) {
    NotificationKind.bookingConfirmed => bookingConfirmed,
    NotificationKind.videoReviewReturned => videoReviewReturned,
    NotificationKind.messageReceived => messageReceived,
  };

  /// [kind] だけを [receives] に変えた設定
  NotificationSettings withKind(NotificationKind kind, bool receives) =>
      NotificationSettings(
        studentId: studentId,
        bookingConfirmed: kind == NotificationKind.bookingConfirmed
            ? receives
            : bookingConfirmed,
        videoReviewReturned: kind == NotificationKind.videoReviewReturned
            ? receives
            : videoReviewReturned,
        messageReceived: kind == NotificationKind.messageReceived
            ? receives
            : messageReceived,
      );

  factory NotificationSettings.fromJson(Map<String, dynamic> json) =>
      NotificationSettings(
        studentId: json['student_id'] as String,
        bookingConfirmed: json['booking_confirmed'] as bool,
        videoReviewReturned: json['video_review_returned'] as bool,
        messageReceived: json['message_received'] as bool,
      );
}

/// `UpdateNotificationSettingsRequest`。書いた種類（null でない項目）だけを変え、書かなかった種類は今の設定のまま
class UpdateNotificationSettingsRequest {
  const UpdateNotificationSettingsRequest({
    this.bookingConfirmed,
    this.videoReviewReturned,
    this.messageReceived,
  });

  /// [kind] 1 つだけを [receives] に変える
  factory UpdateNotificationSettingsRequest.only(
    NotificationKind kind,
    bool receives,
  ) => switch (kind) {
    NotificationKind.bookingConfirmed => UpdateNotificationSettingsRequest(
      bookingConfirmed: receives,
    ),
    NotificationKind.videoReviewReturned => UpdateNotificationSettingsRequest(
      videoReviewReturned: receives,
    ),
    NotificationKind.messageReceived => UpdateNotificationSettingsRequest(
      messageReceived: receives,
    ),
  };

  final bool? bookingConfirmed;
  final bool? videoReviewReturned;
  final bool? messageReceived;

  Map<String, dynamic> toJson() => {
    'booking_confirmed': ?bookingConfirmed,
    'video_review_returned': ?videoReviewReturned,
    'message_received': ?messageReceived,
  };
}

/// `Notification`（glossary の notification。お知らせ）。Flutter の `Notification` と衝突するので [ApiNotification]
class ApiNotification {
  const ApiNotification({
    required this.notificationId,
    required this.kind,
    required this.body,
    required this.createdAt,
    required this.isRead,
  });

  final String notificationId;

  /// 種類。このアプリの版が知らない種類は null（汎用のお知らせとして本文だけを出す）
  final NotificationKind? kind;

  /// 表示用の本文（サーバーが種類と中身から決める）
  final String body;

  /// お知らせを作った時刻
  final DateTime createdAt;

  /// 既読か（`markMyNotificationsRead` で既読にする）
  final bool isRead;

  factory ApiNotification.fromJson(Map<String, dynamic> json) =>
      ApiNotification(
        notificationId: json['notification_id'] as String,
        kind: NotificationKind.tryFromWire(json['kind'] as String),
        body: json['body'] as String,
        createdAt: _dateTime(json['created_at']),
        isRead: json['is_read'] as bool,
      );
}

/// `NotificationList`
class NotificationList {
  /// [notifications] は書き換えられない写しにして持つ
  NotificationList({required List<ApiNotification> notifications})
    : notifications = List.unmodifiable(notifications);

  /// 作った時刻の新しい順
  final List<ApiNotification> notifications;

  factory NotificationList.fromJson(Map<String, dynamic> json) =>
      NotificationList(
        notifications: [
          for (final e in json['notifications'] as List<dynamic>)
            ApiNotification.fromJson(e as Map<String, dynamic>),
        ],
      );
}

/// `Error`
class ApiError {
  const ApiError({required this.code, required this.message});

  final ErrorCode code;

  /// 人間向けの説明（表示用ではない。画面の文言は error_message.dart。表示名の変更の 422 だけはこれを出す）
  final String message;

  factory ApiError.fromJson(Map<String, dynamic> json) => ApiError(
    code: ErrorCode.fromWire(json['code'] as String),
    message: json['message'] as String,
  );
}
