import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';

/// テスト用の [StudentApi]。呼ばれた operationId を [calls] に順に残し、決まった応答を返す。
/// [failures] に operationId を入れると、その操作は [StudentApiException] を投げる。
class FakeStudentApi implements StudentApi {
  FakeStudentApi({
    List<Slot>? slots,
    List<VideoReview>? reviews,
    List<MessageThread>? threads,
    List<Message>? messages,
    NotificationSettings? notificationSettings,
    this.failures = const {},
  }) : slots = slots ?? [Fixtures.slot],
       _reviews = reviews ?? [Fixtures.reviewReturned],
       threads = List.of(threads ?? [Fixtures.thread]),
       messages = List.of(messages ?? Fixtures.messages),
       notificationSettings =
           notificationSettings ?? Fixtures.notificationSettings;

  final List<Slot> slots;
  final List<VideoReview> _reviews;

  /// 担当コーチとのスレッド（`listMessageThreads` が返す）
  final List<MessageThread> threads;

  /// スレッドのメッセージ（`listMessages` が返す。`sendMessage` で末尾に足す）
  final List<Message> messages;

  /// 通知の受け取り設定（`getMyNotificationSettings` が返す。`updateMyNotificationSettings` で書いた種類だけを変える）
  NotificationSettings notificationSettings;
  final Map<String, StudentApiException> failures;

  final List<String> calls = [];
  final List<Object> requests = [];

  Future<T> _answer<T>(
    String operationId,
    T Function() response, [
    Object? request,
  ]) async {
    calls.add(operationId);
    if (request != null) requests.add(request);
    final failure = failures[operationId];
    if (failure != null) throw failure;
    return response();
  }

  @override
  Future<RegisterAccountResponse> registerAccount(
    RegisterAccountRequest body,
  ) => _answer('registerAccount', () => Fixtures.registered, body);

  @override
  Future<SlotPage> searchSlots({required Sport sport, String? cursor}) =>
      _answer(
        'searchSlots',
        () => SlotPage(items: slots.where((s) => s.sport == sport).toList()),
        sport,
      );

  @override
  Future<Booking> createBooking(CreateBookingRequest body) =>
      _answer('createBooking', () => Fixtures.bookingPending, body);

  @override
  Future<Booking> confirmBooking(String bookingId) =>
      _answer('confirmBooking', () => Fixtures.bookingConfirmed, bookingId);

  @override
  Future<LiveLessonSession> startLiveLessonSession(
    StartLiveLessonSessionRequest body,
  ) =>
      _answer('startLiveLessonSession', () => Fixtures.sessionInProgress, body);

  @override
  Future<LiveLessonSession> endLiveLessonSession(String sessionId) =>
      _answer('endLiveLessonSession', () => Fixtures.sessionEnded, sessionId);

  @override
  Future<Video> submitVideo(SubmitVideoRequest body) =>
      _answer('submitVideo', () => Fixtures.video, body);

  @override
  Future<VideoReview> getVideoReview(String videoId) => _answer(
    'getVideoReview',
    () => _reviews.length > 1 ? _reviews.removeAt(0) : _reviews.first,
    videoId,
  );

  @override
  Future<Lesson> recordLessonCompletion(RecordLessonCompletionRequest body) =>
      _answer(
        'recordLessonCompletion',
        () => body is LiveLessonCompletion
            ? Fixtures.liveLesson
            : Fixtures.asyncLesson,
        body,
      );

  @override
  Future<LessonReview> submitLessonReview(SubmitLessonReviewRequest body) =>
      _answer(
        'submitLessonReview',
        () => LessonReview(
          reviewId: Fixtures.reviewId,
          lessonId: body.lessonId,
          studentId: Fixtures.studentId,
          coachId: Fixtures.coachId,
          rating: body.rating,
          comment: body.comment,
          submittedAt: Fixtures.now,
        ),
        body,
      );

  @override
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
  ) => _answer('startSubscription', () => Fixtures.subscriptionStarted, body);

  @override
  Future<MessageThreadList> listMessageThreads() => _answer(
    'listMessageThreads',
    () => MessageThreadList(threads: [...threads]),
  );

  @override
  Future<MessageList> listMessages(String threadId) => _answer(
    'listMessages',
    () => MessageList(threadId: threadId, messages: [...messages]),
    threadId,
  );

  @override
  Future<Message> sendMessage(String threadId, SendMessageRequest body) =>
      _answer('sendMessage', () {
        final message = Message(
          messageId: 'message-${messages.length + 1}',
          threadId: threadId,
          senderId: Fixtures.studentId,
          senderRole: MessageSenderRole.student,
          body: body.body,
          sentAt: Fixtures.now.add(Duration(minutes: messages.length + 1)),
        );
        messages.add(message);
        _updateThread(threadId);
        return message;
      }, body);

  @override
  Future<MessageThread> markMessageThreadRead(String threadId) => _answer(
    'markMessageThreadRead',
    () => _updateThread(threadId, unreadCount: 0),
    threadId,
  );

  @override
  Future<NotificationSettings> getMyNotificationSettings() =>
      _answer('getMyNotificationSettings', () => notificationSettings);

  @override
  Future<NotificationSettings> updateMyNotificationSettings(
    UpdateNotificationSettingsRequest body,
  ) => _answer('updateMyNotificationSettings', () {
    for (final kind in NotificationKind.values) {
      if (body.toJson()[kind.wire] case final bool receives) {
        notificationSettings = notificationSettings.withKind(kind, receives);
      }
    }
    return notificationSettings;
  }, body);

  /// スレッドの最後のメッセージ（と未読の数）を今のメッセージに合わせる
  MessageThread _updateThread(String threadId, {int? unreadCount}) {
    final index = threads.indexWhere((t) => t.threadId == threadId);
    final thread = threads[index];
    return threads[index] = MessageThread(
      threadId: thread.threadId,
      studentId: thread.studentId,
      coachId: thread.coachId,
      openedAt: thread.openedAt,
      lastMessage: messages.lastOrNull,
      unreadCount: unreadCount ?? thread.unreadCount,
    );
  }
}

/// 契約の形に沿った応答の見本
abstract final class Fixtures {
  static const studentId = '11111111-1111-4111-8111-111111111111';
  static const coachId = '22222222-2222-4222-8222-222222222222';
  static const slotId = '33333333-3333-4333-8333-333333333333';
  static const bookingId = '44444444-4444-4444-8444-444444444444';
  static const sessionId = '55555555-5555-4555-8555-555555555555';
  static const videoId = '66666666-6666-4666-8666-666666666666';
  static const threadId = 'cccccccc-cccc-4ccc-8ccc-cccccccccccc';
  static const reviewId = 'ffffffff-ffff-4fff-8fff-ffffffffffff';

  /// 評価済み（`lesson_already_reviewed`）の 409
  static const alreadyReviewed = StudentApiException(
    statusCode: 409,
    error: ApiError(
      code: ErrorCode.lessonAlreadyReviewed,
      message: 'lesson already reviewed',
    ),
  );

  /// 入力の不備（`validation_failed`）の 422
  static const validationFailed = StudentApiException(
    statusCode: 422,
    error: ApiError(
      code: ErrorCode.validationFailed,
      message: 'validation failed',
    ),
  );

  /// 通信の失敗（契約の `Error` なし）
  static const unreachable = StudentApiException();

  /// まだ変えていない通知の受け取り設定（既定はすべて受け取る）
  static const notificationSettings = NotificationSettings(
    studentId: studentId,
    bookingConfirmed: true,
    videoReviewReturned: true,
    messageReceived: true,
  );

  static final now = DateTime.utc(2026, 10, 26, 3);

  static final student = Student(
    studentId: studentId,
    displayName: '山田',
    sport: Sport.tennis,
    createdAt: now,
  );

  static final registered = RegisterAccountResponse(
    student: student,
    accessToken: 'mock.student.$studentId',
  );

  static final slot = Slot(
    slotId: slotId,
    coachId: coachId,
    sport: Sport.tennis,
    startsAt: now.add(const Duration(days: 2)),
    endsAt: now.add(const Duration(days: 2, hours: 1)),
  );

  static final bookingPending = Booking(
    bookingId: bookingId,
    slotId: slotId,
    studentId: studentId,
    coachId: coachId,
    isTrial: true,
    status: BookingStatus.pending,
    startsAt: slot.startsAt,
    createdAt: now,
  );

  static final bookingConfirmed = Booking(
    bookingId: bookingId,
    slotId: slotId,
    studentId: studentId,
    coachId: coachId,
    isTrial: true,
    status: BookingStatus.confirmed,
    startsAt: slot.startsAt,
    createdAt: now,
    confirmedAt: now,
  );

  static final sessionInProgress = LiveLessonSession(
    sessionId: sessionId,
    bookingId: bookingId,
    studentId: studentId,
    coachId: coachId,
    status: LiveLessonSessionStatus.inProgress,
    startedAt: now,
    roomToken: 'mock-room-token',
  );

  static final sessionEnded = LiveLessonSession(
    sessionId: sessionId,
    bookingId: bookingId,
    studentId: studentId,
    coachId: coachId,
    status: LiveLessonSessionStatus.ended,
    startedAt: now,
    endedAt: now.add(const Duration(minutes: 30)),
    durationSec: 1800,
  );

  static final video = Video(
    videoId: videoId,
    studentId: studentId,
    coachId: coachId,
    sport: Sport.tennis,
    status: VideoStatus.awaitingUpload,
    uploadUrl: 'https://objectstore.invalid/upload',
    createdAt: now,
  );

  static const reviewInProgress = VideoReview(
    videoId: videoId,
    studentId: studentId,
    coachId: coachId,
    status: VideoReviewStatus.inProgress,
  );

  static final reviewReturned = VideoReview(
    reviewId: '77777777-7777-4777-8777-777777777777',
    videoId: videoId,
    studentId: studentId,
    coachId: coachId,
    status: VideoReviewStatus.returned,
    comment: 'テイクバックをもう少し早く',
    returnedAt: now,
  );

  static final liveLesson = Lesson(
    lessonId: '88888888-8888-4888-8888-888888888888',
    studentId: studentId,
    coachId: coachId,
    lessonType: LessonType.liveLesson,
    sessionId: sessionId,
    isFirstLesson: true,
    completedAt: now,
  );

  static final asyncLesson = Lesson(
    lessonId: '99999999-9999-4999-8999-999999999999',
    studentId: studentId,
    coachId: coachId,
    lessonType: LessonType.asyncLesson,
    videoId: videoId,
    isFirstLesson: true,
    completedAt: now,
  );

  static final subscriptionStarted = StartSubscriptionResponse(
    subscription: Subscription(
      subscriptionId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      studentId: studentId,
      sport: Sport.tennis,
      status: SubscriptionStatus.active,
      startedAt: now,
    ),
    entitlement: const Entitlement(
      entitlementId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
      studentId: studentId,
      contractType: ContractType.subscription,
      sport: 'tennis',
      unlimitedVideoAndChat: true,
      liveLessonQuota: LiveLessonQuota(
        month: '2026-10',
        limit: 2,
        used: 0,
        remaining: 2,
      ),
    ),
  );

  /// 自分が送ってコーチが読んだメッセージと、コーチから届いてまだ読んでいないメッセージ
  static final messages = [
    Message(
      messageId: 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
      threadId: threadId,
      senderId: studentId,
      senderRole: MessageSenderRole.student,
      body: 'よろしくお願いします',
      sentAt: now,
      readAt: now.add(const Duration(minutes: 1)),
    ),
    Message(
      messageId: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
      threadId: threadId,
      senderId: coachId,
      senderRole: MessageSenderRole.coach,
      body: '体験レッスンでお待ちしています',
      sentAt: now.add(const Duration(minutes: 2)),
    ),
  ];

  static final thread = MessageThread(
    threadId: threadId,
    studentId: studentId,
    coachId: coachId,
    openedAt: now,
    lastMessage: messages.last,
    unreadCount: 1,
  );
}
