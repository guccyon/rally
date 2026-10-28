import 'models.dart';

/// student-app の契約の操作。メソッドは operationId と 1 対 1（Gen1 の一周シナリオ・レッスンの評価・チャット・通知の受け取り設定で使うものだけ）。
///
/// 失敗は [StudentApiException] で返す。画面のテストはこの抽象をモックに差し替える。
abstract interface class StudentApi {
  /// `registerAccount`（POST /accounts）
  Future<RegisterAccountResponse> registerAccount(RegisterAccountRequest body);

  /// `searchSlots`（GET /slots）
  Future<SlotPage> searchSlots({required Sport sport, String? cursor});

  /// `createBooking`（POST /bookings）
  Future<Booking> createBooking(CreateBookingRequest body);

  /// `confirmBooking`（POST /bookings/{booking_id}/confirm）
  Future<Booking> confirmBooking(String bookingId);

  /// `startLiveLessonSession`（POST /live-lesson-sessions）
  Future<LiveLessonSession> startLiveLessonSession(
    StartLiveLessonSessionRequest body,
  );

  /// `endLiveLessonSession`（POST /live-lesson-sessions/{session_id}/end）
  Future<LiveLessonSession> endLiveLessonSession(String sessionId);

  /// `submitVideo`（POST /videos）
  Future<Video> submitVideo(SubmitVideoRequest body);

  /// `getVideoReview`（GET /videos/{video_id}/review）
  Future<VideoReview> getVideoReview(String videoId);

  /// `recordLessonCompletion`（POST /lessons/completions）
  Future<Lesson> recordLessonCompletion(RecordLessonCompletionRequest body);

  /// `submitLessonReview`（POST /lesson-reviews）。2 回目は 409 `lesson_already_reviewed`
  Future<LessonReview> submitLessonReview(SubmitLessonReviewRequest body);

  /// `startSubscription`（POST /subscriptions）
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
  );

  /// `listMessageThreads`（GET /message-threads）
  Future<MessageThreadList> listMessageThreads();

  /// `listMessages`（GET /message-threads/{thread_id}/messages）。読むだけでは既読にならない
  Future<MessageList> listMessages(String threadId);

  /// `sendMessage`（POST /message-threads/{thread_id}/messages）
  Future<Message> sendMessage(String threadId, SendMessageRequest body);

  /// `markMessageThreadRead`（POST /message-threads/{thread_id}/read）
  Future<MessageThread> markMessageThreadRead(String threadId);

  /// `getMyNotificationSettings`（GET /notification-settings/me）。まだ変えていなければすべて true
  Future<NotificationSettings> getMyNotificationSettings();

  /// `updateMyNotificationSettings`（PUT /notification-settings/me）。書いた種類だけを変え、変えたあとの設定を返す
  Future<NotificationSettings> updateMyNotificationSettings(
    UpdateNotificationSettingsRequest body,
  );
}

/// API の失敗。契約の `Error` を返したときは [error] を持ち、通信の失敗などでは null。
class StudentApiException implements Exception {
  const StudentApiException({this.statusCode, this.error});

  final int? statusCode;
  final ApiError? error;

  @override
  String toString() =>
      'StudentApiException($statusCode, ${error?.code.wire}: ${error?.message})';
}
