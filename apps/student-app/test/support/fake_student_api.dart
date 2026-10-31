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
    List<MyBooking>? myBookings,
    List<MyVideo>? myVideos,
    this.entitlement,
    this.coachReviewSummaries = const {},
    this.lessonMenus = const {},
    this.videoReviews = const {},
    this.bookingLessonMenu,
    this.failures = const {},
  }) : slots = slots ?? [Fixtures.slot],
       myBookings = List.of(myBookings ?? Fixtures.myBookings),
       myVideos = List.of(myVideos ?? Fixtures.myVideos),
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

  /// 自分の予約（`listMyBookings` が返す。`cancelBooking` でその予約をキャンセル済みにする）
  final List<MyBooking> myBookings;

  /// 自分が送った動画（`listMyVideos` が返す）
  final List<MyVideo> myVideos;

  /// 受講権（`getMyEntitlement` が返す。null なら月額契約の前として 404 `not_found`。`startSubscription` で開始の応答のものになる）
  Entitlement? entitlement;

  /// コーチの ID ごとの評価の件数と平均（`getCoachReviewSummary` が返す。ないコーチは [Fixtures.coachReviews] の 2 件で平均 4.5）
  final Map<String, CoachReviewSummary> coachReviewSummaries;

  /// コーチの ID ごとの公開中のレッスンメニュー（`listCoachLessonMenus` が `sport` で絞って返す。ないコーチは空の一覧）
  final Map<String, List<LessonMenu>> lessonMenus;

  /// 動画の ID ごとの添削（`getVideoReview` はここにある動画ならこれを返し、ない動画は `reviews` を順に返す）
  final Map<String, VideoReview> videoReviews;

  /// `createBooking` / `confirmBooking` が返す予約に載せる、予約した時点のレッスンメニューの写し（null ならメニューなし）
  final BookingLessonMenu? bookingLessonMenu;

  /// `getVideoReview` だけを、動画の ID ごとに失敗させる
  final Set<String> videoReviewFailures = {};
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
  ) => _answer(
    'registerAccount',
    () => RegisterAccountResponse(
      // 登録したスポーツの生徒にする（それ以外は見本のとおり）
      student: Student(
        studentId: Fixtures.student.studentId,
        displayName: Fixtures.student.displayName,
        sport: body.sport,
        createdAt: Fixtures.student.createdAt,
      ),
      accessToken: Fixtures.registered.accessToken,
    ),
    body,
  );

  @override
  Future<SlotPage> searchSlots({required Sport sport, String? cursor}) =>
      _answer(
        'searchSlots',
        () => SlotPage(items: slots.where((s) => s.sport == sport).toList()),
        sport,
      );

  @override
  Future<Booking> createBooking(CreateBookingRequest body) => _answer(
    'createBooking',
    () => Fixtures.withLessonMenu(Fixtures.bookingPending, bookingLessonMenu),
    body,
  );

  @override
  Future<Booking> confirmBooking(String bookingId) => _answer(
    'confirmBooking',
    () => Fixtures.withLessonMenu(Fixtures.bookingConfirmed, bookingLessonMenu),
    bookingId,
  );

  @override
  Future<MyBookingList> listMyBookings() =>
      _answer('listMyBookings', () => MyBookingList(bookings: [...myBookings]));

  @override
  Future<Booking> cancelBooking(String bookingId) =>
      _answer('cancelBooking', () {
        final index = myBookings.indexWhere(
          (b) => b.booking.bookingId == bookingId,
        );
        final item = myBookings[index];
        final cancelled = Fixtures.bookingWith(
          item.booking,
          status: BookingStatus.cancelled,
          cancelledAt: Fixtures.now,
        );
        myBookings[index] = MyBooking(booking: cancelled, slot: item.slot);
        return cancelled;
      }, bookingId);

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
      _answer('submitVideo', () {
        // サーバーと同じく、その生徒とコーチのスレッドに添削依頼のメッセージを足す
        final index = threads.indexWhere((t) => t.coachId == body.coachId);
        if (index >= 0) {
          final threadId = threads[index].threadId;
          messages.add(
            Message(
              messageId: 'message-${messages.length + 1}',
              threadId: threadId,
              senderId: Fixtures.studentId,
              senderRole: MessageSenderRole.student,
              kind: MessageKind.videoReviewRequest,
              body: '動画の添削を依頼しました',
              videoId: Fixtures.video.videoId,
              sentAt: Fixtures.now.add(Duration(minutes: messages.length + 1)),
            ),
          );
          _updateThread(threadId);
        }
        return Fixtures.video;
      }, body);

  @override
  Future<MyVideoList> listMyVideos() =>
      _answer('listMyVideos', () => MyVideoList(videos: [...myVideos]));

  @override
  Future<VideoReview> getVideoReview(String videoId) =>
      _answer('getVideoReview', () {
        if (videoReviewFailures.contains(videoId)) throw Fixtures.unreachable;
        return videoReviews[videoId] ??
            (_reviews.length > 1 ? _reviews.removeAt(0) : _reviews.first);
      }, videoId);

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
  Future<CoachReviewSummary> getCoachReviewSummary(String coachId) => _answer(
    'getCoachReviewSummary',
    () =>
        coachReviewSummaries[coachId] ??
        CoachReviewSummary(
          coachId: coachId,
          reviewCount: Fixtures.coachReviews.length,
          averageRating: 4.5,
        ),
    coachId,
  );

  @override
  Future<CoachReviewList> listCoachReviews(String coachId) => _answer(
    'listCoachReviews',
    () => CoachReviewList(coachId: coachId, reviews: Fixtures.coachReviews),
    coachId,
  );

  @override
  Future<LessonMenuList> listCoachLessonMenus(String coachId, {Sport? sport}) =>
      _answer(
        'listCoachLessonMenus',
        () => LessonMenuList(
          coachId: coachId,
          lessonMenus: [
            for (final menu in lessonMenus[coachId] ?? const <LessonMenu>[])
              if (sport == null || menu.sport == sport) menu,
          ],
        ),
        (coachId: coachId, sport: sport),
      );

  @override
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
  ) => _answer('startSubscription', () {
    final response = Fixtures.subscriptionStartedWith(
      body.plan ?? SubscriptionPlan.standard,
    );
    entitlement = response.entitlement;
    return response;
  }, body);

  @override
  Future<Entitlement> getMyEntitlement() => _answer(
    'getMyEntitlement',
    () => entitlement ?? (throw Fixtures.entitlementNotFound),
  );

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
          kind: body.stampId == null ? MessageKind.text : MessageKind.stamp,
          body: body.body ?? 'スタンプ: ${body.stampId?.wire}',
          stampId: body.stampId,
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

  /// 受講権がまだない（月額契約の前）ときの `getMyEntitlement` の 404
  static const entitlementNotFound = StudentApiException(
    statusCode: 404,
    error: ApiError(code: ErrorCode.notFound, message: 'not found'),
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

  /// [slot] に、コーチの名前と自己紹介文と、結びついたレッスンメニュー（どれも任意）を添えたもの
  static Slot slotWithCoach({
    String? displayName,
    String? bio,
    SlotLessonMenu? lessonMenu,
  }) => Slot(
    slotId: slot.slotId,
    coachId: slot.coachId,
    sport: slot.sport,
    startsAt: slot.startsAt,
    endsAt: slot.endsAt,
    coachDisplayName: displayName,
    coachBio: bio,
    lessonMenu: lessonMenu,
  );

  /// [slot] に結びついたレッスンメニュー（[lessonMenus] の先頭のテニスの同期。1 回の時間は枠の長さと同じ 60 分）
  static final slotLessonMenu = SlotLessonMenu(
    lessonMenuId: lessonMenus.first.lessonMenuId,
    name: lessonMenus.first.name,
    durationMinutes: lessonMenus.first.durationMinutes,
  );

  /// 予約した時点の [slotLessonMenu] の写し（`Booking.lesson_menu`）
  static final bookingLessonMenu = BookingLessonMenu(
    lessonMenuId: slotLessonMenu.lessonMenuId,
    name: slotLessonMenu.name,
    durationMinutes: slotLessonMenu.durationMinutes,
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

  /// [booking] の状態とキャンセルの時刻を変えたもの
  static Booking bookingWith(
    Booking booking, {
    required BookingStatus status,
    DateTime? cancelledAt,
  }) => Booking(
    bookingId: booking.bookingId,
    slotId: booking.slotId,
    studentId: booking.studentId,
    coachId: booking.coachId,
    isTrial: booking.isTrial,
    status: status,
    startsAt: booking.startsAt,
    createdAt: booking.createdAt,
    confirmedAt: booking.confirmedAt,
    cancelledAt: cancelledAt ?? booking.cancelledAt,
    lessonMenu: booking.lessonMenu,
  );

  /// [booking] に予約した時点のレッスンメニューの写しを載せたもの（[lessonMenu] が null ならメニューなし）
  static Booking withLessonMenu(
    Booking booking,
    BookingLessonMenu? lessonMenu,
  ) => Booking(
    bookingId: booking.bookingId,
    slotId: booking.slotId,
    studentId: booking.studentId,
    coachId: booking.coachId,
    isTrial: booking.isTrial,
    status: booking.status,
    startsAt: booking.startsAt,
    createdAt: booking.createdAt,
    confirmedAt: booking.confirmedAt,
    cancelledAt: booking.cancelledAt,
    lessonMenu: lessonMenu,
  );

  /// 自分の予約の一覧の 1 件。スロットは [now] から [startsIn] 後に始まる 1 時間（過去なら負の値）
  static MyBooking myBooking({
    required String bookingId,
    required Duration startsIn,
    BookingStatus status = BookingStatus.confirmed,
    Sport sport = Sport.tennis,
    String? coachDisplayName = '佐藤',
    BookingLessonMenu? lessonMenu,
  }) {
    final startsAt = now.add(startsIn);
    return MyBooking(
      booking: Booking(
        bookingId: bookingId,
        slotId: 'slot-$bookingId',
        studentId: studentId,
        coachId: coachId,
        isTrial: false,
        status: status,
        startsAt: startsAt,
        createdAt: now.subtract(const Duration(days: 7)),
        confirmedAt: status == BookingStatus.pending ? null : now,
        cancelledAt: status == BookingStatus.cancelled ? now : null,
        lessonMenu: lessonMenu,
      ),
      slot: Slot(
        slotId: 'slot-$bookingId',
        coachId: coachId,
        sport: sport,
        startsAt: startsAt,
        endsAt: startsAt.add(const Duration(hours: 1)),
        coachDisplayName: coachDisplayName,
      ),
    );
  }

  /// 3 日後（確定済み）・12 時間後（確定待ち）・6 時間後（キャンセル済み）・2 日前（確定済みで終わったもの）の 4 件（開始時刻の新しい順）
  static final myBookings = [
    myBooking(bookingId: 'booking-later', startsIn: const Duration(days: 3)),
    myBooking(
      bookingId: 'booking-soon',
      startsIn: const Duration(hours: 12),
      status: BookingStatus.pending,
      sport: Sport.soccer,
      coachDisplayName: '鈴木',
    ),
    myBooking(
      bookingId: 'booking-cancelled',
      startsIn: const Duration(hours: 6),
      status: BookingStatus.cancelled,
    ),
    myBooking(bookingId: 'booking-past', startsIn: const Duration(days: -2)),
  ];

  /// 自分の動画の一覧の 1 件。[now] から [submittedAgo] 前に送った動画
  static MyVideo myVideo({
    required String videoId,
    required Duration submittedAgo,
    VideoReviewStatus reviewStatus = VideoReviewStatus.inProgress,
    Sport sport = Sport.tennis,
    String? coachDisplayName = '佐藤',
    VideoLessonMenu? lessonMenu,
  }) => MyVideo(
    videoId: videoId,
    coachId: coachId,
    coachDisplayName: coachDisplayName,
    sport: sport,
    submittedAt: now.subtract(submittedAgo),
    reviewStatus: reviewStatus,
    returnedAt: reviewStatus == VideoReviewStatus.returned ? now : null,
    lessonMenu: lessonMenu,
  );

  /// 1 時間前（添削待ち・メニューあり）・2 日前（返却済み・サッカー・コーチ鈴木）の 2 件（送った時刻の新しい順）
  static final myVideos = [
    myVideo(
      videoId: 'video-waiting',
      submittedAgo: const Duration(hours: 1),
      lessonMenu: videoLessonMenu,
    ),
    myVideo(
      videoId: 'video-returned',
      submittedAgo: const Duration(days: 2),
      reviewStatus: VideoReviewStatus.returned,
      sport: Sport.soccer,
      coachDisplayName: '鈴木',
    ),
  ];

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

  static final aiFeedback = AiFeedback(
    comment: 'フォームの全体の流れは安定しています',
    createdAt: now,
  );

  static final reviewInProgressWithAi = VideoReview(
    videoId: videoId,
    studentId: studentId,
    coachId: coachId,
    status: VideoReviewStatus.inProgress,
    aiFeedback: aiFeedback,
  );

  static final reviewReturnedWithAi = VideoReview(
    reviewId: reviewReturned.reviewId,
    videoId: videoId,
    studentId: studentId,
    coachId: coachId,
    status: VideoReviewStatus.returned,
    comment: reviewReturned.comment,
    returnedAt: now,
    aiFeedback: aiFeedback,
  );

  /// 動画を送った時点で選んだレッスンメニューの写し（[lessonMenus] のテニスの非同期）
  static final videoLessonMenu = VideoLessonMenu(
    lessonMenuId: lessonMenus[1].lessonMenuId,
    name: lessonMenus[1].name,
  );

  /// 送るときにレッスンメニューを選んだ動画の、返却された添削
  static final reviewReturnedWithMenu = VideoReview(
    reviewId: reviewReturned.reviewId,
    videoId: videoId,
    studentId: studentId,
    coachId: coachId,
    status: VideoReviewStatus.returned,
    comment: reviewReturned.comment,
    returnedAt: now,
    lessonMenu: videoLessonMenu,
  );

  /// コーチが受けた評価（送った時刻の新しい順）
  static final coachReviews = [
    CoachReview(
      reviewId: reviewId,
      lessonId: '77777777-7777-4777-8777-777777777777',
      rating: 5,
      comment: '丁寧でした',
      submittedAt: now,
    ),
    CoachReview(
      reviewId: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
      lessonId: '88888888-8888-4888-8888-888888888888',
      rating: 4,
      comment: 'わかりやすかった',
      submittedAt: now.subtract(const Duration(days: 1)),
    ),
  ];

  /// コーチの公開中のレッスンメニュー（作った時刻の古い順）。テニスの同期・非同期と、野球の同期
  static final lessonMenus = [
    LessonMenu(
      lessonMenuId: '12121212-1212-4212-8212-121212121212',
      coachId: coachId,
      sport: Sport.tennis,
      lessonType: LessonType.liveLesson,
      name: 'フォアハンドの基礎',
      description: '打点とスイングの軌道を見直します',
      durationMinutes: 60,
      isPublished: true,
      createdAt: now.subtract(const Duration(days: 2)),
    ),
    LessonMenu(
      lessonMenuId: '13131313-1313-4313-8313-131313131313',
      coachId: coachId,
      sport: Sport.tennis,
      lessonType: LessonType.asyncLesson,
      name: 'サーブの動画添削',
      description: '',
      isPublished: true,
      createdAt: now.subtract(const Duration(days: 1)),
    ),
    LessonMenu(
      lessonMenuId: '14141414-1414-4414-8414-141414141414',
      coachId: coachId,
      sport: Sport.baseball,
      lessonType: LessonType.liveLesson,
      name: 'バッティングの基礎',
      description: '構えから見直します',
      durationMinutes: 30,
      isPublished: true,
      createdAt: now,
    ),
  ];

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

  static final subscriptionStarted = subscriptionStartedWith(
    SubscriptionPlan.standard,
  );

  /// [plan] で始めた月額契約。今月のビデオ通話はまだ使っていない（上限は standard 2 回・plus 4 回）
  static StartSubscriptionResponse subscriptionStartedWith(
    SubscriptionPlan plan,
  ) {
    final limit = plan == SubscriptionPlan.plus ? 4 : 2;
    return StartSubscriptionResponse(
      subscription: Subscription(
        subscriptionId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        studentId: studentId,
        sport: Sport.tennis,
        plan: plan,
        status: SubscriptionStatus.active,
        startedAt: now,
      ),
      entitlement: Entitlement(
        entitlementId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
        studentId: studentId,
        contractType: ContractType.subscription,
        sport: 'tennis',
        plan: plan,
        unlimitedVideoAndChat: true,
        liveLessonQuota: LiveLessonQuota(
          month: '2026-10',
          limit: limit,
          used: 0,
          remaining: limit,
        ),
      ),
    );
  }

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
