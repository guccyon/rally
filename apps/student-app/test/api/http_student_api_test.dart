import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:rally_student_app/api/error_message.dart';
import 'package:rally_student_app/api/http_student_api.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';

void main() {
  late List<http.Request> sent;

  HttpStudentApi apiReplying(
    int status,
    Object? body, {
    String? token = 'mock.student.x',
  }) {
    sent = [];
    return HttpStudentApi(
      baseUrl: Uri.parse('http://api.test'),
      accessToken: () => token,
      client: MockClient((request) async {
        sent.add(request);
        return http.Response.bytes(
          body == null ? [] : utf8.encode(jsonEncode(body)),
          status,
          headers: {'content-type': 'application/json'},
        );
      }),
    );
  }

  test('registerAccount はトークンを付けずに POST /accounts へ送る', () async {
    final api = apiReplying(201, {
      'student': {
        'student_id': 's1',
        'display_name': '山田',
        'sport': 'tennis',
        'created_at': '2026-10-26T03:00:00Z',
      },
      'access_token': 'mock.student.s1',
    });

    final response = await api.registerAccount(
      const RegisterAccountRequest(
        email: 'a@example.com',
        displayName: '山田',
        sport: Sport.tennis,
      ),
    );

    expect(sent.single.method, 'POST');
    expect(sent.single.url.toString(), 'http://api.test/accounts');
    expect(sent.single.headers.containsKey('Authorization'), isFalse);
    expect(jsonDecode(sent.single.body), {
      'email': 'a@example.com',
      'display_name': '山田',
      'sport': 'tennis',
    });
    expect(response.accessToken, 'mock.student.s1');
    expect(response.student.displayName, '山田');
  });

  test('searchSlots は sport で絞り、Bearer トークンを付ける', () async {
    final api = apiReplying(200, {
      'items': [
        {
          'slot_id': 'sl1',
          'coach_id': 'c1',
          'sport': 'soccer',
          'starts_at': '2026-10-28T03:00:00Z',
          'ends_at': '2026-10-28T04:00:00Z',
        },
      ],
      'next_cursor': null,
    });

    final page = await api.searchSlots(sport: Sport.soccer);

    expect(sent.single.method, 'GET');
    expect(sent.single.url.toString(), 'http://api.test/slots?sport=soccer');
    expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
    expect(page.items.single.sport, Sport.soccer);
    expect(page.nextCursor, isNull);
  });

  test('操作ごとのパス', () async {
    final booking = {
      'booking_id': 'b1',
      'slot_id': 'sl1',
      'student_id': 's1',
      'coach_id': 'c1',
      'is_trial': true,
      'status': 'confirmed',
      'starts_at': '2026-10-28T03:00:00Z',
      'created_at': '2026-10-26T03:00:00Z',
      'confirmed_at': '2026-10-26T03:01:00Z',
      'cancelled_at': null,
    };
    expect(
      (await apiReplying(200, booking).confirmBooking('b1')).status,
      BookingStatus.confirmed,
    );
    expect(sent.single.url.path, '/bookings/b1/confirm');

    final session = {
      'session_id': 'ls1',
      'booking_id': 'b1',
      'student_id': 's1',
      'coach_id': 'c1',
      'status': 'ended',
      'started_at': '2026-10-28T03:00:00Z',
      'ended_at': '2026-10-28T03:30:00Z',
      'duration_sec': 1800,
      'room_token': null,
    };
    expect(
      (await apiReplying(200, session).endLiveLessonSession('ls1')).durationSec,
      1800,
    );
    expect(sent.single.url.path, '/live-lesson-sessions/ls1/end');

    final review = {
      'review_id': null,
      'video_id': 'v1',
      'student_id': 's1',
      'coach_id': 'c1',
      'status': 'in_progress',
      'comment': null,
      'returned_at': null,
    };
    expect(
      (await apiReplying(200, review).getVideoReview('v1')).status,
      VideoReviewStatus.inProgress,
    );
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/videos/v1/review');

    final lesson = {
      'lesson_id': 'l1',
      'student_id': 's1',
      'coach_id': 'c1',
      'lesson_type': 'async_lesson',
      'session_id': null,
      'video_id': 'v1',
      'is_first_lesson': true,
      'completed_at': '2026-10-28T03:30:00Z',
    };
    final recorded = await apiReplying(
      201,
      lesson,
    ).recordLessonCompletion(const AsyncLessonCompletion(videoId: 'v1'));
    expect(sent.single.url.path, '/lessons/completions');
    expect(jsonDecode(sent.single.body), {
      'lesson_type': 'async_lesson',
      'video_id': 'v1',
    });
    expect(recorded.isFirstLesson, isTrue);
  });

  test('チャットの操作のパスと、メッセージ・スレッドの変換', () async {
    final coachMessage = {
      'message_id': 'm1',
      'thread_id': 't1',
      'sender_id': 'c1',
      'sender_role': 'coach',
      'body': 'こんにちは',
      'sent_at': '2026-10-26T03:00:00Z',
      'read_at': null,
    };
    final thread = {
      'thread_id': 't1',
      'student_id': 's1',
      'coach_id': 'c1',
      'opened_at': '2026-10-26T02:00:00Z',
      'last_message': coachMessage,
      'unread_count': 1,
    };

    final threads = await apiReplying(200, {
      'threads': [
        thread,
        {...thread, 'thread_id': 't2', 'last_message': null},
      ],
    }).listMessageThreads();
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/message-threads');
    expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
    expect(
      threads.threads.first.lastMessage!.senderRole,
      MessageSenderRole.coach,
    );
    expect(threads.threads.first.unreadCount, 1);
    expect(threads.threads.last.lastMessage, isNull);

    final list = await apiReplying(200, {
      'thread_id': 't1',
      'messages': [coachMessage],
    }).listMessages('t1');
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/message-threads/t1/messages');
    expect(list.messages.single.readAt, isNull);

    final message = await apiReplying(201, {
      ...coachMessage,
      'message_id': 'm2',
      'sender_id': 's1',
      'sender_role': 'student',
      'body': 'よろしく',
    }).sendMessage('t1', const SendMessageRequest(body: 'よろしく'));
    expect(sent.single.method, 'POST');
    expect(sent.single.url.path, '/message-threads/t1/messages');
    expect(jsonDecode(sent.single.body), {'body': 'よろしく'});
    expect(message.senderRole, MessageSenderRole.student);

    final read = await apiReplying(200, {
      ...thread,
      'last_message': {...coachMessage, 'read_at': '2026-10-26T03:05:00Z'},
      'unread_count': 0,
    }).markMessageThreadRead('t1');
    expect(sent.single.method, 'POST');
    expect(sent.single.url.path, '/message-threads/t1/read');
    expect(read.unreadCount, 0);
    expect(read.lastMessage!.readAt, DateTime.utc(2026, 10, 26, 3, 5));
  });

  test('契約の Error を StudentApiException にし、画面の文言に変える', () async {
    final api = apiReplying(409, {
      'code': 'live_lesson_quota_exceeded',
      'message': 'quota exceeded',
    });

    final error = await api
        .confirmBooking('b1')
        .then<Object?>((_) => null, onError: (Object e) => e);

    expect(error, isA<StudentApiException>());
    final exception = error as StudentApiException;
    expect(exception.statusCode, 409);
    expect(exception.error!.code, ErrorCode.liveLessonQuotaExceeded);
    expect(errorMessageOf(exception), '今月のビデオ通話の回数（2 回）を使い切っています。');
  });

  test('知らない ErrorCode は汎用のエラーとして扱う', () async {
    final api = apiReplying(409, {'code': 'something_new', 'message': '...'});

    final error = await api
        .confirmBooking('b1')
        .then<Object?>((_) => null, onError: (Object e) => e);

    expect((error as StudentApiException).error!.code, ErrorCode.unknown);
    expect(errorMessageOf(error), 'エラーが起きました。時間をおいてやり直してください。');
  });
}
