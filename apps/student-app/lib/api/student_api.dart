import 'models.dart';

/// student-app の契約の操作。メソッドは operationId と 1 対 1（Gen1 の一周シナリオ・自分の予約・自分の動画・受講権の照会・レッスンの評価・コーチを探す・コーチの紹介・コーチの評価・お気に入りのコーチ・契約プランの一覧・チャット・通知の受け取り設定・お知らせで使うものだけ）。
///
/// 失敗は [StudentApiException] で返す。画面のテストはこの抽象をモックに差し替える。
abstract interface class StudentApi {
  /// `registerAccount`（POST /accounts）
  Future<RegisterAccountResponse> registerAccount(RegisterAccountRequest body);

  /// `searchSlots`（GET /slots）。[coachId] を渡せばそのコーチの枠に絞る
  Future<SlotPage> searchSlots({
    required Sport sport,
    String? coachId,
    String? cursor,
  });

  /// `createBooking`（POST /bookings）
  Future<Booking> createBooking(CreateBookingRequest body);

  /// `confirmBooking`（POST /bookings/{booking_id}/confirm）
  Future<Booking> confirmBooking(String bookingId);

  /// `listMyBookings`（GET /bookings/me）。キャンセル済みも含み、スロットの開始時刻の新しい順
  Future<MyBookingList> listMyBookings();

  /// `cancelBooking`（POST /bookings/{booking_id}/cancel）。本文（任意の `reason`）は送らない。
  /// 回数を消費するかはサーバー（scheduling）が判定し、応答には含まれない
  Future<Booking> cancelBooking(String bookingId);

  /// `startLiveLessonSession`（POST /live-lesson-sessions）
  Future<LiveLessonSession> startLiveLessonSession(
    StartLiveLessonSessionRequest body,
  );

  /// `endLiveLessonSession`（POST /live-lesson-sessions/{session_id}/end）
  Future<LiveLessonSession> endLiveLessonSession(String sessionId);

  /// `submitVideo`（POST /videos）
  Future<Video> submitVideo(SubmitVideoRequest body);

  /// `listMyVideos`（GET /videos/me）。添削待ちも返却済みも含み、送った時刻の新しい順
  Future<MyVideoList> listMyVideos();

  /// `getVideoReview`（GET /videos/{video_id}/review）
  Future<VideoReview> getVideoReview(String videoId);

  /// `recordLessonCompletion`（POST /lessons/completions）
  Future<Lesson> recordLessonCompletion(RecordLessonCompletionRequest body);

  /// `listMyLessonCompletions`（GET /lessons/completions/me）。通話も動画添削も含み、完了した時刻の新しい順
  Future<MyLessonCompletionList> listMyLessonCompletions();

  /// `submitLessonReview`（POST /lesson-reviews）。2 回目は 409 `lesson_already_reviewed`
  Future<LessonReview> submitLessonReview(SubmitLessonReviewRequest body);

  /// `listCoachesForStudent`（GET /coaches）。[sport] を省けば生徒自身のスポーツのコーチ。並びはサーバーが決め、ページングはしない
  Future<CoachListingList> listCoachesForStudent({Sport? sport});

  /// `getCoachProfile`（GET /coaches/{coach_id}）。表示名・自己紹介文（書いていなければ null）・提供スポーツ。
  /// コーチでないアカウント・ない ID・ゴルフだけのコーチは 404
  Future<CoachProfile> getCoachProfile(String coachId);

  /// `getCoachReviewSummary`（GET /coaches/{coach_id}/review-summary）。どのコーチの分も読める。評価がなければ 0 件で平均は null
  Future<CoachReviewSummary> getCoachReviewSummary(String coachId);

  /// `listCoachReviews`（GET /coaches/{coach_id}/reviews）。送った時刻の新しい順。評価がなければ空
  Future<CoachReviewList> listCoachReviews(String coachId);

  /// `listCoachLessonMenus`（GET /coaches/{coach_id}/lesson-menus）。コーチの公開中のレッスンメニューだけを、作った時刻の
  /// 古い順で返す。[sport] を渡せばそのスポーツに絞る。メニューがなければ空
  Future<LessonMenuList> listCoachLessonMenus(String coachId, {Sport? sport});

  /// `listFavoriteCoaches`（GET /favorite-coaches）。お気に入りに入れた時刻の新しい順。なければ空
  Future<FavoriteCoachList> listFavoriteCoaches();

  /// `addFavoriteCoach`（PUT /favorite-coaches/{coach_id}）。何度入れても 1 件で、入れたあとの一覧を返す。
  /// `getCoachProfile` が 404 を返すコーチは 404
  Future<FavoriteCoachList> addFavoriteCoach(String coachId);

  /// `removeFavoriteCoach`（DELETE /favorite-coaches/{coach_id}）。入っていなくても 200 で、外したあとの一覧を返す
  Future<FavoriteCoachList> removeFavoriteCoach(String coachId);

  /// `listSubscriptionPlans`（GET /subscription-plans）。契約プランとビデオ通話の暦月の上限を standard・plus の順で返す。
  /// 受講権がなくても（月額契約の前でも）呼べる
  Future<SubscriptionPlanList> listSubscriptionPlans();

  /// `startSubscription`（POST /subscriptions）
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
  );

  /// `getMyEntitlement`（GET /entitlements/me）。受講権がまだない（月額契約の前）なら 404
  Future<Entitlement> getMyEntitlement();

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

  /// `listMyNotifications`（GET /notifications/me）。作った時刻の新しい順。読むだけでは既読にならない
  Future<NotificationList> listMyNotifications();

  /// `markMyNotificationsRead`（POST /notifications/me/read）。すべて既読にし、既読にしたあとの一覧を返す
  Future<NotificationList> markMyNotificationsRead();

  /// `getMyStudentProfile`（GET /me/profile）。トークンが示す本人（生徒）のアカウント
  Future<Student> getMyStudentProfile();

  /// `updateMyStudentProfile`（PATCH /me/profile）。表示名を置き換え、置き換えたあとの本人のアカウントを返す
  Future<Student> updateMyStudentProfile(UpdateStudentProfileRequest body);
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
