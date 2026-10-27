// student-app の契約（specs/contracts/api/student-app/openapi.yaml）の components.schemas を手で写したもの。
// 型名・項目名は契約のスキーマ名に揃える（Dart の命名規約に合わせて snake_case は lowerCamelCase に）。
// JSON との変換はこのファイルだけで行う。契約の enum の値とのずれは test/api/contract_test.dart が検出する。

/// `Sport`。student-app が扱うスポーツ（ゴルフは golf-app の範囲）。
enum Sport {
  tennis('tennis'),
  baseball('baseball'),
  soccer('soccer'),
  basketball('basketball');

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

/// `MessageSenderRole`（スレッドのどちらの当事者が送ったか）
enum MessageSenderRole {
  student('student'),
  coach('coach');

  const MessageSenderRole(this.wire);
  final String wire;

  static MessageSenderRole fromWire(String value) =>
      _fromWire(values, value, (e) => e.wire);
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
  });

  final String slotId;
  final String coachId;
  final Sport sport;
  final DateTime startsAt;
  final DateTime endsAt;

  factory Slot.fromJson(Map<String, dynamic> json) => Slot(
    slotId: json['slot_id'] as String,
    coachId: json['coach_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    startsAt: _dateTime(json['starts_at']),
    endsAt: _dateTime(json['ends_at']),
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
  );
}

/// `CreateBookingRequest`
class CreateBookingRequest {
  const CreateBookingRequest({required this.slotId, required this.isTrial});

  final String slotId;
  final bool isTrial;

  Map<String, dynamic> toJson() => {'slot_id': slotId, 'is_trial': isTrial};
}

/// `LiveLessonQuota`
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
    this.unlimitedVideoAndChat,
    this.liveLessonQuota,
    this.remainingVideoReviews,
  });

  final String entitlementId;
  final String studentId;
  final ContractType contractType;

  /// 月額は Sport のいずれか、ワンショットは golf なので文字列のまま持つ（契約も string）。
  final String sport;
  final bool? unlimitedVideoAndChat;
  final LiveLessonQuota? liveLessonQuota;
  final int? remainingVideoReviews;

  factory Entitlement.fromJson(Map<String, dynamic> json) => Entitlement(
    entitlementId: json['entitlement_id'] as String,
    studentId: json['student_id'] as String,
    contractType: ContractType.fromWire(json['contract_type'] as String),
    sport: json['sport'] as String,
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
    required this.status,
    required this.startedAt,
  });

  final String subscriptionId;
  final String studentId;
  final Sport sport;
  final SubscriptionStatus status;
  final DateTime startedAt;

  factory Subscription.fromJson(Map<String, dynamic> json) => Subscription(
    subscriptionId: json['subscription_id'] as String,
    studentId: json['student_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    status: SubscriptionStatus.fromWire(json['status'] as String),
    startedAt: _dateTime(json['started_at']),
  );
}

/// `StartSubscriptionRequest`
class StartSubscriptionRequest {
  const StartSubscriptionRequest({
    required this.sport,
    required this.paymentMethodToken,
  });

  final Sport sport;

  /// 決済手段のトークン（Gen1 はモック。サーバーは受け取るだけで使わない）
  final String paymentMethodToken;

  Map<String, dynamic> toJson() => {
    'sport': sport.wire,
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
  });

  final String videoId;
  final String studentId;
  final String coachId;
  final Sport sport;
  final VideoStatus status;

  /// 動画本体の PUT 先（Gen1 はモック）
  final String? uploadUrl;
  final DateTime createdAt;

  factory Video.fromJson(Map<String, dynamic> json) => Video(
    videoId: json['video_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    sport: Sport.fromWire(json['sport'] as String),
    status: VideoStatus.fromWire(json['status'] as String),
    uploadUrl: json['upload_url'] as String?,
    createdAt: _dateTime(json['created_at']),
  );
}

/// `SubmitVideoRequest`
class SubmitVideoRequest {
  const SubmitVideoRequest({
    required this.coachId,
    required this.sport,
    required this.contentType,
    this.note,
  });

  final String coachId;
  final Sport sport;
  final VideoContentType contentType;

  /// コーチへの相談内容（1000 文字まで）
  final String? note;

  Map<String, dynamic> toJson() => {
    'coach_id': coachId,
    'sport': sport.wire,
    'content_type': contentType.wire,
    if (note != null) 'note': note,
  };
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
  });

  final String? reviewId;
  final String videoId;
  final String studentId;
  final String coachId;
  final VideoReviewStatus status;
  final String? comment;
  final DateTime? returnedAt;

  factory VideoReview.fromJson(Map<String, dynamic> json) => VideoReview(
    reviewId: json['review_id'] as String?,
    videoId: json['video_id'] as String,
    studentId: json['student_id'] as String,
    coachId: json['coach_id'] as String,
    status: VideoReviewStatus.fromWire(json['status'] as String),
    comment: json['comment'] as String?,
    returnedAt: _dateTimeOrNull(json['returned_at']),
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

/// `Message`（glossary の message。送信時刻と既読を持つ）
class Message {
  const Message({
    required this.messageId,
    required this.threadId,
    required this.senderId,
    required this.senderRole,
    required this.body,
    required this.sentAt,
    this.readAt,
  });

  final String messageId;
  final String threadId;

  /// 送ったアカウント（スレッドの生徒か担当コーチ）
  final String senderId;
  final MessageSenderRole senderRole;
  final String body;
  final DateTime sentAt;

  /// 受け取った側が読んだ時刻（既読）。読むまでは null
  final DateTime? readAt;

  factory Message.fromJson(Map<String, dynamic> json) => Message(
    messageId: json['message_id'] as String,
    threadId: json['thread_id'] as String,
    senderId: json['sender_id'] as String,
    senderRole: MessageSenderRole.fromWire(json['sender_role'] as String),
    body: json['body'] as String,
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

  /// 最後のメッセージ。まだなければ null
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

  /// 送った順
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

/// `SendMessageRequest`
class SendMessageRequest {
  /// 本文が空白だけ、または [maxBodyLength] 文字を超えるときは [ArgumentError]
  factory SendMessageRequest({required String body}) {
    if (body.trim().isEmpty) {
      throw ArgumentError.value(body, 'body', '空白だけの本文は送れません');
    }
    // 契約の maxLength（JSON Schema）は Unicode のコードポイントで数える
    if (body.runes.length > maxBodyLength) {
      throw ArgumentError.value(body, 'body', '本文は $maxBodyLength 文字までです');
    }
    return SendMessageRequest._(body);
  }

  const SendMessageRequest._(this.body);

  /// 契約の `SendMessageRequest.body` の maxLength
  static const maxBodyLength = 2000;

  /// 本文（2000 文字まで。空白だけは不可）
  final String body;

  Map<String, dynamic> toJson() => {'body': body};
}

/// `Error`
class ApiError {
  const ApiError({required this.code, required this.message});

  final ErrorCode code;

  /// 人間向けの説明（表示用ではない。画面の文言は error_message.dart）
  final String message;

  factory ApiError.fromJson(Map<String, dynamic> json) => ApiError(
    code: ErrorCode.fromWire(json['code'] as String),
    message: json['message'] as String,
  );
}
