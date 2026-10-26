import 'dart:convert';

import 'package:http/http.dart' as http;

import 'models.dart';
import 'student_api.dart';

/// [StudentApi] の HTTP 実装。トークンは不透明なものとして `Authorization: Bearer` に載せるだけ。
class HttpStudentApi implements StudentApi {
  HttpStudentApi({
    required this._baseUrl,
    required this._accessToken,
    http.Client? client,
  }) : _client = client ?? http.Client();

  final Uri _baseUrl;
  final String? Function() _accessToken;
  final http.Client _client;

  @override
  Future<RegisterAccountResponse> registerAccount(
    RegisterAccountRequest body,
  ) async => RegisterAccountResponse.fromJson(
    await _send('POST', '/accounts', body: body.toJson(), authorized: false),
  );

  @override
  Future<SlotPage> searchSlots({required Sport sport, String? cursor}) async =>
      SlotPage.fromJson(
        await _send(
          'GET',
          '/slots',
          query: {'sport': sport.wire, 'cursor': ?cursor},
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
  Future<VideoReview> getVideoReview(String videoId) async =>
      VideoReview.fromJson(
        await _send('GET', '/videos/${Uri.encodeComponent(videoId)}/review'),
      );

  @override
  Future<Lesson> recordLessonCompletion(
    RecordLessonCompletionRequest body,
  ) async => Lesson.fromJson(
    await _send('POST', '/lessons/completions', body: body.toJson()),
  );

  @override
  Future<StartSubscriptionResponse> startSubscription(
    StartSubscriptionRequest body,
  ) async => StartSubscriptionResponse.fromJson(
    await _send('POST', '/subscriptions', body: body.toJson()),
  );

  Future<Map<String, dynamic>> _send(
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

    final http.Response response;
    try {
      response = await http.Response.fromStream(await _client.send(request));
    } on http.ClientException {
      throw const StudentApiException();
    }

    final decoded = _decode(response);
    if (response.statusCode >= 200 && response.statusCode < 300) {
      if (decoded is Map<String, dynamic>) return decoded;
      throw StudentApiException(statusCode: response.statusCode);
    }
    throw StudentApiException(
      statusCode: response.statusCode,
      error: decoded is Map<String, dynamic> && decoded['code'] is String
          ? ApiError.fromJson(decoded)
          : null,
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
