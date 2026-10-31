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

  test('searchSlots は sport で絞り、Bearer トークンを付け、枠のレッスンメニューを読む', () async {
    final api = apiReplying(200, {
      'items': [
        {
          'slot_id': 'sl1',
          'coach_id': 'c1',
          'sport': 'soccer',
          'starts_at': '2026-10-28T03:00:00Z',
          'ends_at': '2026-10-28T04:00:00Z',
          'capacity': 1,
          'remaining_seats': 1,
          'lesson_menu': {
            'lesson_menu_id': 'm1',
            'name': 'シュートの基礎',
            'duration_minutes': 60,
          },
        },
        {
          'slot_id': 'sl2',
          'coach_id': 'c1',
          'sport': 'soccer',
          'starts_at': '2026-10-29T03:00:00Z',
          'ends_at': '2026-10-29T04:00:00Z',
          'capacity': 1,
          'remaining_seats': 1,
          'lesson_menu': null,
        },
      ],
      'next_cursor': null,
    });

    final page = await api.searchSlots(sport: Sport.soccer);

    expect(sent.single.method, 'GET');
    expect(sent.single.url.toString(), 'http://api.test/slots?sport=soccer');
    expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
    expect(page.items.map((s) => s.sport), [Sport.soccer, Sport.soccer]);
    expect(page.items.first.lessonMenu!.lessonMenuId, 'm1');
    expect(page.items.first.lessonMenu!.name, 'シュートの基礎');
    expect(page.items.first.lessonMenu!.durationMinutes, 60);
    expect(page.items.last.lessonMenu, isNull);
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

    expect(
      (await apiReplying(200, {
        ...booking,
        'status': 'cancelled',
        'cancelled_at': '2026-10-27T03:00:00Z',
      }).cancelBooking('b1')).status,
      BookingStatus.cancelled,
    );
    expect(sent.single.method, 'POST');
    expect(sent.single.url.path, '/bookings/b1/cancel');
    expect(sent.single.body, isEmpty);

    final myBookings = await apiReplying(200, {
      'bookings': [
        {
          'booking': booking,
          'slot': {
            'slot_id': 'sl1',
            'coach_id': 'c1',
            'sport': 'tennis',
            'starts_at': '2026-10-28T03:00:00Z',
            'ends_at': '2026-10-28T04:00:00Z',
            'capacity': 1,
            'remaining_seats': 1,
            'coach_display_name': '佐藤',
          },
        },
      ],
    }).listMyBookings();
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/bookings/me');
    expect(myBookings.bookings.single.slot.coachDisplayName, '佐藤');

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

  test('submitLessonReview は POST /lesson-reviews へ送り、評価を変換する', () async {
    final review =
        await apiReplying(201, {
          'review_id': 'r1',
          'lesson_id': 'l1',
          'student_id': 's1',
          'coach_id': 'c1',
          'rating': 4,
          'comment': '丁寧でした',
          'submitted_at': '2026-10-28T04:00:00Z',
        }).submitLessonReview(
          const SubmitLessonReviewRequest(
            lessonId: 'l1',
            rating: 4,
            comment: '丁寧でした',
          ),
        );

    expect(sent.single.method, 'POST');
    expect(sent.single.url.path, '/lesson-reviews');
    expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
    expect(jsonDecode(sent.single.body), {
      'lesson_id': 'l1',
      'rating': 4,
      'comment': '丁寧でした',
    });
    expect(review.coachId, 'c1');
    expect(review.rating, 4);
    expect(review.submittedAt, DateTime.utc(2026, 10, 28, 4));

    final error =
        await apiReplying(409, {
              'code': 'lesson_already_reviewed',
              'message': 'already reviewed',
            })
            .submitLessonReview(
              const SubmitLessonReviewRequest(
                lessonId: 'l1',
                rating: 4,
                comment: '丁寧でした',
              ),
            )
            .then<Object?>((_) => null, onError: (Object e) => e);
    expect(
      (error as StudentApiException).error!.code,
      ErrorCode.lessonAlreadyReviewed,
    );
  });

  test('コーチの評価の件数と平均・一覧は GET /coaches/{coach_id}/… へ送り、変換する', () async {
    final summary = await apiReplying(200, {
      'coach_id': 'c1',
      'review_count': 3,
      'average_rating': 4,
    }).getCoachReviewSummary('c1');
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/coaches/c1/review-summary');
    expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
    expect(summary.coachId, 'c1');
    expect(summary.reviewCount, 3);
    // JSON の整数でも小数として読む
    expect(summary.averageRating, 4.0);

    final empty = await apiReplying(200, {
      'coach_id': 'c2',
      'review_count': 0,
      'average_rating': null,
    }).getCoachReviewSummary('c2');
    expect(empty.reviewCount, 0);
    expect(empty.averageRating, isNull);

    final list = await apiReplying(200, {
      'coach_id': 'c1',
      'reviews': [
        {
          'review_id': 'r2',
          'lesson_id': 'l2',
          'rating': 5,
          'comment': '丁寧でした',
          'submitted_at': '2026-10-28T04:00:00Z',
        },
        {
          'review_id': 'r1',
          'lesson_id': 'l1',
          'rating': 3,
          'comment': 'ふつう',
          'submitted_at': '2026-10-27T04:00:00Z',
        },
      ],
    }).listCoachReviews('c1');
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/coaches/c1/reviews');
    expect(list.coachId, 'c1');
    // 返った順（新しい順）のまま
    expect(list.reviews.map((r) => r.reviewId), ['r2', 'r1']);
    expect(list.reviews.first.rating, 5);
    expect(list.reviews.first.comment, '丁寧でした');
    expect(list.reviews.first.submittedAt, DateTime.utc(2026, 10, 28, 4));

    final none = await apiReplying(200, {
      'coach_id': 'c2',
      'reviews': <Object>[],
    }).listCoachReviews('c2');
    expect(none.reviews, isEmpty);
  });

  test('listCoachLessonMenus のパスとスポーツの絞り込み、レッスンメニューの変換', () async {
    final live = {
      'lesson_menu_id': 'm1',
      'coach_id': 'c1',
      'sport': 'tennis',
      'lesson_type': 'live_lesson',
      'name': 'フォア改善',
      'description': '初心者向け',
      'duration_minutes': 45,
      'is_published': true,
      'created_at': '2026-10-30T01:00:00Z',
    };
    final asyncMenu = {
      ...live,
      'lesson_menu_id': 'm2',
      'lesson_type': 'async_lesson',
      'name': 'フォームの添削',
      'description': '',
      'duration_minutes': null,
    };
    final list = await apiReplying(200, {
      'coach_id': 'c1',
      'lesson_menus': [live, asyncMenu],
    }).listCoachLessonMenus('c1');
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/coaches/c1/lesson-menus');
    expect(sent.single.url.queryParameters, isEmpty);
    expect(list.coachId, 'c1');
    // 返った順のまま
    expect(list.lessonMenus.map((m) => m.lessonMenuId), ['m1', 'm2']);
    final first = list.lessonMenus.first;
    expect(first.sport, Sport.tennis);
    expect(first.lessonType, LessonType.liveLesson);
    expect(first.name, 'フォア改善');
    expect(first.description, '初心者向け');
    expect(first.durationMinutes, 45);
    expect(first.isPublished, isTrue);
    expect(first.createdAt, DateTime.utc(2026, 10, 30, 1));
    expect(list.lessonMenus.last.lessonType, LessonType.asyncLesson);
    expect(list.lessonMenus.last.durationMinutes, isNull);

    final none = await apiReplying(200, {
      'coach_id': 'c1',
      'lesson_menus': <Object>[],
    }).listCoachLessonMenus('c1', sport: Sport.swimming);
    expect(sent.single.url.path, '/coaches/c1/lesson-menus');
    expect(sent.single.url.queryParameters, {'sport': 'swimming'});
    expect(none.lessonMenus, isEmpty);
  });

  test('チャットの操作のパスと、メッセージ・スレッドの変換', () async {
    final coachMessage = {
      'message_id': 'm1',
      'thread_id': 't1',
      'sender_id': 'c1',
      'sender_role': 'coach',
      'kind': 'text',
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
    }).sendMessage('t1', SendMessageRequest(body: 'よろしく'));
    expect(sent.single.method, 'POST');
    expect(sent.single.url.path, '/message-threads/t1/messages');
    expect(jsonDecode(sent.single.body), {'body': 'よろしく'});
    expect(message.senderRole, MessageSenderRole.student);

    final stamp = await apiReplying(201, {
      ...coachMessage,
      'sender_id': 's1',
      'sender_role': 'student',
      'kind': 'stamp',
      'stamp_id': 'thanks',
      'body': 'スタンプ: ありがとう',
    }).sendMessage('t1', SendMessageRequest.stamp(StampId.thanks));
    expect(sent.single.url.path, '/message-threads/t1/messages');
    expect(jsonDecode(sent.single.body), {'stamp_id': 'thanks'});
    expect(stamp.kind, MessageKind.stamp);
    expect(stamp.stampId, StampId.thanks);

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

  test('通知の受け取り設定の操作のパスと、書いた種類だけを送る変更', () async {
    final settings = {
      'student_id': 's1',
      'booking_confirmed': true,
      'video_review_returned': true,
      'message_received': true,
    };

    final read = await apiReplying(200, settings).getMyNotificationSettings();
    expect(sent.single.method, 'GET');
    expect(sent.single.url.path, '/notification-settings/me');
    expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
    expect(read.studentId, 's1');
    expect(NotificationKind.values.map(read.receives), everyElement(isTrue));

    final updated =
        await apiReplying(200, {
          ...settings,
          'video_review_returned': false,
        }).updateMyNotificationSettings(
          UpdateNotificationSettingsRequest.only(
            NotificationKind.videoReviewReturned,
            false,
          ),
        );
    expect(sent.single.method, 'PUT');
    expect(sent.single.url.path, '/notification-settings/me');
    expect(jsonDecode(sent.single.body), {'video_review_returned': false});
    expect(updated.videoReviewReturned, isFalse);
    expect(updated.bookingConfirmed, isTrue);
    expect(updated.messageReceived, isTrue);
  });

  test(
    'getMyEntitlement は GET /entitlements/me で受講権を読み、受講権がなければ 404',
    () async {
      final entitlement = await apiReplying(200, {
        'entitlement_id': 'e1',
        'student_id': 's1',
        'contract_type': 'subscription',
        'sport': 'tennis',
        'plan': 'plus',
        'unlimited_video_and_chat': true,
        'live_lesson_quota': {
          'month': '2026-10',
          'limit': 4,
          'used': 1,
          'remaining': 3,
        },
        'remaining_video_reviews': null,
      }).getMyEntitlement();
      expect(sent.single.method, 'GET');
      expect(sent.single.url.path, '/entitlements/me');
      expect(sent.single.headers['Authorization'], 'Bearer mock.student.x');
      expect(entitlement.plan, SubscriptionPlan.plus);
      expect(entitlement.liveLessonQuota!.remaining, 3);

      final error = await apiReplying(
        404,
        {'code': 'not_found', 'message': 'entitlement not found'},
      ).getMyEntitlement().then<Object?>((_) => null, onError: (Object e) => e);
      expect((error as StudentApiException).statusCode, 404);
      expect(error.error!.code, ErrorCode.notFound);
    },
  );

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
