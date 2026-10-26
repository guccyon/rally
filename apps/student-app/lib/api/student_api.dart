import 'models.dart';

/// student-app の契約の操作。メソッドは operationId と 1 対 1（Gen1 の一周シナリオで使うものだけ）。
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

  /// `startSubscription`（POST /subscriptions）
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
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
