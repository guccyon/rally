import 'dart:convert';

import 'package:http/http.dart' as http;

import 'models.dart';
import 'server_clock.dart';
import 'student_api.dart';

/// [StudentApi] の HTTP 実装。トークンは不透明なものとして `Authorization: Bearer` に載せるだけ。
/// 応答の Date ヘッダーから出したサーバーの時計とのずれ（[clockOffsetOf]）を、`listMyBookings` の応答と失敗（[StudentApiException]）に添える。
class HttpStudentApi implements StudentApi {
  HttpStudentApi({
    required this._baseUrl,
    required this._accessToken,
    http.Client? client,
    DateTime Function()? deviceNow,
  }) : _client = client ?? http.Client(),
       _deviceNow = deviceNow ?? DateTime.now;

  final Uri _baseUrl;
  final String? Function() _accessToken;
  final http.Client _client;

  /// 端末の時計（送った時刻・受け取った時刻。テストで差し替える）
  final DateTime Function() _deviceNow;

  @override
  Future<RegisterAccountResponse> registerAccount(
    RegisterAccountRequest body,
  ) async => RegisterAccountResponse.fromJson(
    await _send('POST', '/accounts', body: body.toJson(), authorized: false),
  );

  @override
  Future<SlotPage> searchSlots({
    required Sport sport,
    String? coachId,
    String? cursor,
  }) async => SlotPage.fromJson(
    await _send(
      'GET',
      '/slots',
      query: {'sport': sport.wire, 'coach_id': ?coachId, 'cursor': ?cursor},
    ),
  );

  @override
  Future<Booking> createBooking(CreateBookingRequest body) async =>
      Booking.fromJson(await _send('POST', '/bookings', body: body.toJson()));

  @override
  Future<Booking> confirmBooking(String bookingId) async => Booking.fromJson(
    await _send('POST', '/bookings/${Uri.encodeComponent(bookingId)}/confirm'),
  );

  @override
  Future<MyBookingList> listMyBookings() async {
    final (:json, :clockOffset) = await _sendTimed('GET', '/bookings/me');
    return MyBookingList.fromJson(json, clockOffset: clockOffset);
  }

  @override
  Future<Booking> cancelBooking(String bookingId) async => Booking.fromJson(
    await _send('POST', '/bookings/${Uri.encodeComponent(bookingId)}/cancel'),
  );

  @override
  Future<LiveLessonSession> startLiveLessonSession(
    StartLiveLessonSessionRequest body,
  ) async => LiveLessonSession.fromJson(
    await _send('POST', '/live-lesson-sessions', body: body.toJson()),
  );

  @override
  Future<LiveLessonSession> endLiveLessonSession(String sessionId) async =>
      LiveLessonSession.fromJson(
        await _send(
          'POST',
          '/live-lesson-sessions/${Uri.encodeComponent(sessionId)}/end',
        ),
      );

  @override
  Future<Video> submitVideo(SubmitVideoRequest body) async =>
      Video.fromJson(await _send('POST', '/videos', body: body.toJson()));

  @override
  Future<MyVideoList> listMyVideos() async =>
      MyVideoList.fromJson(await _send('GET', '/videos/me'));

  @override
  Future<VideoReview> getVideoReview(String videoId) async =>
      VideoReview.fromJson(
        await _send('GET', '/videos/${Uri.encodeComponent(videoId)}/review'),
      );

  @override
  Future<MyLessonCompletionList> listMyLessonCompletions() async =>
      MyLessonCompletionList.fromJson(
        await _send('GET', '/lessons/completions/me'),
      );

  @override
  Future<Lesson> recordLessonCompletion(
    RecordLessonCompletionRequest body,
  ) async => Lesson.fromJson(
    await _send('POST', '/lessons/completions', body: body.toJson()),
  );

  @override
  Future<LessonReview> submitLessonReview(
    SubmitLessonReviewRequest body,
  ) async => LessonReview.fromJson(
    await _send('POST', '/lesson-reviews', body: body.toJson()),
  );

  @override
  Future<CoachListingList> listCoachesForStudent({Sport? sport}) async =>
      CoachListingList.fromJson(
        await _send('GET', '/coaches', query: {'sport': ?sport?.wire}),
      );

  @override
  Future<CoachProfile> getCoachProfile(String coachId) async =>
      CoachProfile.fromJson(
        await _send('GET', '/coaches/${Uri.encodeComponent(coachId)}'),
      );

  @override
  Future<CoachReviewSummary> getCoachReviewSummary(String coachId) async =>
      CoachReviewSummary.fromJson(
        await _send(
          'GET',
          '/coaches/${Uri.encodeComponent(coachId)}/review-summary',
        ),
      );

  @override
  Future<CoachReviewList> listCoachReviews(String coachId) async =>
      CoachReviewList.fromJson(
        await _send('GET', '/coaches/${Uri.encodeComponent(coachId)}/reviews'),
      );

  @override
  Future<LessonMenuList> listCoachLessonMenus(
    String coachId, {
    Sport? sport,
  }) async => LessonMenuList.fromJson(
    await _send(
      'GET',
      '/coaches/${Uri.encodeComponent(coachId)}/lesson-menus',
      query: {'sport': ?sport?.wire},
    ),
  );

  /// 契約に反する応答（`live_lessons_per_month` が 1 未満・契約にないプランなど、[FormatException] になるもの）は、
  /// 呼び出し側が失敗として受け止められるよう [StudentApiException] に包み直す
  @override
  Future<SubscriptionPlanList> listSubscriptionPlans() async {
    final json = await _send('GET', '/subscription-plans');
    try {
      return SubscriptionPlanList.fromJson(json);
    } on FormatException {
      throw const StudentApiException(statusCode: 200);
    }
  }

  @override
  Future<FavoriteCoachList> listFavoriteCoaches() async =>
      _favoriteCoachList(await _send('GET', '/favorite-coaches'));

  @override
  Future<FavoriteCoachList> addFavoriteCoach(String coachId) async =>
      _favoriteCoachList(
        await _send('PUT', '/favorite-coaches/${Uri.encodeComponent(coachId)}'),
      );

  @override
  Future<FavoriteCoachList> removeFavoriteCoach(String coachId) async =>
      _favoriteCoachList(
        await _send(
          'DELETE',
          '/favorite-coaches/${Uri.encodeComponent(coachId)}',
        ),
      );

  /// お気に入りのコーチの一覧の応答を読む。契約に反する応答（契約にないスポーツ・項目の欠けや型の違いなど、
  /// [FormatException] や [TypeError] になるもの）は、呼び出し側（ホームの欄・コーチの紹介）が失敗として受け止められるよう
  /// [StudentApiException] に包み直す
  static FavoriteCoachList _favoriteCoachList(Map<String, dynamic> json) {
    try {
      return FavoriteCoachList.fromJson(json);
    } on FormatException {
      throw const StudentApiException(statusCode: 200);
    } on TypeError {
      throw const StudentApiException(statusCode: 200);
    }
  }

  @override
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
  ) async => StartSubscriptionResponse.fromJson(
    await _send('POST', '/subscriptions', body: body.toJson()),
  );

  @override
  Future<Entitlement> getMyEntitlement() async =>
      Entitlement.fromJson(await _send('GET', '/entitlements/me'));

  @override
  Future<MessageThreadList> listMessageThreads() async =>
      MessageThreadList.fromJson(await _send('GET', '/message-threads'));

  @override
  Future<MessageList> listMessages(String threadId) async =>
      MessageList.fromJson(
        await _send(
          'GET',
          '/message-threads/${Uri.encodeComponent(threadId)}/messages',
        ),
      );

  @override
  Future<Message> sendMessage(String threadId, SendMessageRequest body) async =>
      Message.fromJson(
        await _send(
          'POST',
          '/message-threads/${Uri.encodeComponent(threadId)}/messages',
          body: body.toJson(),
        ),
      );

  @override
  Future<MessageThread> markMessageThreadRead(String threadId) async =>
      MessageThread.fromJson(
        await _send(
          'POST',
          '/message-threads/${Uri.encodeComponent(threadId)}/read',
        ),
      );

  @override
  Future<NotificationSettings> getMyNotificationSettings() async =>
      NotificationSettings.fromJson(
        await _send('GET', '/notification-settings/me'),
      );

  @override
  Future<NotificationSettings> updateMyNotificationSettings(
    UpdateNotificationSettingsRequest body,
  ) async => NotificationSettings.fromJson(
    await _send('PUT', '/notification-settings/me', body: body.toJson()),
  );

  @override
  Future<NotificationList> listMyNotifications() async =>
      NotificationList.fromJson(await _send('GET', '/notifications/me'));

  @override
  Future<NotificationList> markMyNotificationsRead() async =>
      NotificationList.fromJson(await _send('POST', '/notifications/me/read'));

  @override
  Future<Student> getMyStudentProfile() async =>
      Student.fromJson(await _send('GET', '/me/profile'));

  @override
  Future<Student> updateMyStudentProfile(
    UpdateStudentProfileRequest body,
  ) async => Student.fromJson(
    await _send('PATCH', '/me/profile', body: body.toJson()),
  );

  @override
  Future<NextActionList> getMyNextActions() async =>
      NextActionList.fromJson(await _send('GET', '/me/next-actions'));

  Future<Map<String, dynamic>> _send(
    String method,
    String path, {
    Map<String, String>? query,
    Map<String, dynamic>? body,
    bool authorized = true,
  }) async => (await _sendTimed(
    method,
    path,
    query: query,
    body: body,
    authorized: authorized,
  )).json;

  /// 送って、応答の本体と、応答の Date から出したずれ（Date がない・読めなければ null）を返す。失敗のずれは [StudentApiException.clockOffset]
  Future<({Map<String, dynamic> json, Duration? clockOffset})> _sendTimed(
    String method,
    String path, {
    Map<String, String>? query,
    Map<String, dynamic>? body,
    bool authorized = true,
  }) async {
    final request = http.Request(
      method,
      _baseUrl.replace(
        path: '${_baseUrl.path.replaceFirst(RegExp(r'/$'), '')}$path',
        queryParameters: query == null || query.isEmpty ? null : query,
      ),
    );
    request.headers['Accept'] = 'application/json';
    final token = authorized ? _accessToken() : null;
    if (token != null) {
      request.headers['Authorization'] = 'Bearer $token';
    }
    if (body != null) {
      request.headers['Content-Type'] = 'application/json';
      request.body = jsonEncode(body);
    }

    final sentAt = _deviceNow();
    final http.StreamedResponse streamed;
    try {
      streamed = await _client.send(request);
    } on http.ClientException {
      throw const StudentApiException();
    }
    // 頭が届いた時刻で測る（本体を読み終えるまでの時間はずれに入れない）
    final clockOffset = clockOffsetOf(
      streamed.headers['date'],
      sentAt: sentAt,
      receivedAt: _deviceNow(),
    );
    final http.Response response;
    try {
      response = await http.Response.fromStream(streamed);
    } on http.ClientException {
      throw const StudentApiException();
    }

    final decoded = _decode(response);
    if (response.statusCode >= 200 && response.statusCode < 300) {
      if (decoded is Map<String, dynamic>) {
        return (json: decoded, clockOffset: clockOffset);
      }
      throw StudentApiException(
        statusCode: response.statusCode,
        clockOffset: clockOffset,
      );
    }
    throw StudentApiException(
      statusCode: response.statusCode,
      error: decoded is Map<String, dynamic> && decoded['code'] is String
          ? ApiError.fromJson(decoded)
          : null,
      clockOffset: clockOffset,
    );
  }

  static Object? _decode(http.Response response) {
    if (response.body.isEmpty) return null;
    try {
      return jsonDecode(utf8.decode(response.bodyBytes));
    } on FormatException {
      return null;
    }
  }
}
