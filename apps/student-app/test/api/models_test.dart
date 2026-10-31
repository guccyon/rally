import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';

void main() {
  group('SendMessageRequest', () {
    test('2000 文字までの本文は作れる', () {
      final body = 'あ' * SendMessageRequest.maxBodyLength;
      expect(SendMessageRequest(body: body).toJson(), {'body': body});
    });

    test('空・空白だけの本文は ArgumentError', () {
      expect(() => SendMessageRequest(body: ''), throwsArgumentError);
      expect(() => SendMessageRequest(body: ' 　\n'), throwsArgumentError);
    });

    test('2000 文字を超える本文は ArgumentError（コードポイントで数える）', () {
      expect(() => SendMessageRequest(body: 'あ' * 2001), throwsArgumentError);
      // サロゲートペアの文字は 1 文字と数える
      expect(SendMessageRequest(body: '😀' * 2000).body, '😀' * 2000);
      expect(() => SendMessageRequest(body: '😀' * 2001), throwsArgumentError);
    });

    test('本文の代わりにスタンプを送る（本文は書かない）', () {
      final request = SendMessageRequest.stamp(StampId.niceShot);
      expect(request.toJson(), {'stamp_id': 'nice_shot'});
      expect(request.body, isNull);
      expect(SendMessageRequest(body: 'x').stampId, isNull);
    });
  });

  final message = {
    'message_id': 'm1',
    'thread_id': 't1',
    'sender_id': 'c1',
    'sender_role': 'coach',
    'kind': 'text',
    'body': 'こんにちは',
    'sent_at': '2026-10-26T03:00:00Z',
    'read_at': null,
  };

  group('Message の種類', () {
    test('本文は動画・添削を指さない', () {
      final parsed = Message.fromJson(message);
      expect(parsed.kind, MessageKind.text);
      expect(parsed.body, 'こんにちは');
      expect(parsed.videoId, isNull);
      expect(parsed.reviewId, isNull);
    });

    test('添削依頼は動画の ID、添削の返却は動画の ID と添削の ID を持つ', () {
      final request = Message.fromJson({
        ...message,
        'sender_id': 's1',
        'sender_role': 'student',
        'kind': 'video_review_request',
        'body': '動画の添削を依頼しました',
        'video_id': 'v1',
      });
      expect(request.kind, MessageKind.videoReviewRequest);
      expect(request.senderRole, MessageSenderRole.student);
      expect(request.videoId, 'v1');
      expect(request.reviewId, isNull);

      final returned = Message.fromJson({
        ...message,
        'kind': 'video_review_returned',
        'body': '動画の添削を返しました',
        'video_id': 'v1',
        'review_id': 'r1',
      });
      expect(returned.kind, MessageKind.videoReviewReturned);
      expect(returned.videoId, 'v1');
      expect(returned.reviewId, 'r1');
    });

    test('システムが流したメッセージを読む', () {
      final parsed = Message.fromJson({
        ...message,
        'sender_id': '00000000-0000-0000-0000-000000000000',
        'sender_role': 'system',
        'body': '年末年始はお休みです',
      });
      expect(parsed.senderRole, MessageSenderRole.system);
    });

    test('スタンプは種類とスタンプの ID と、古い版向けの本文の説明を読む', () {
      final parsed = Message.fromJson({
        ...message,
        'kind': 'stamp',
        'stamp_id': 'got_it',
        'body': 'スタンプ: 了解',
      });
      expect(parsed.kind, MessageKind.stamp);
      expect(parsed.stampId, StampId.gotIt);
      expect(parsed.body, 'スタンプ: 了解');
      expect(parsed.videoId, isNull);
      expect(Message.fromJson(message).stampId, isNull);
    });

    test('知らないスタンプの ID は null として読む（本文の説明は残る）', () {
      final parsed = Message.fromJson({
        ...message,
        'kind': 'stamp',
        'stamp_id': 'heart',
        'body': 'スタンプ: ハート',
      });
      expect(parsed.kind, MessageKind.stamp);
      expect(parsed.stampId, isNull);
      expect(parsed.body, 'スタンプ: ハート');
      for (final stamp in StampId.values) {
        expect(StampId.tryFromWire(stamp.wire), stamp);
      }
    });

    final unknown = {...message, 'message_id': 'm9', 'kind': 'sticker'};

    test('知らない種類のメッセージは読み飛ばさず、種類「その他」として本文（短い説明）ごと読む', () {
      final list = MessageList.fromJson({
        'thread_id': 't1',
        'messages': [message, unknown],
      });
      expect(list.messages.map((m) => m.messageId), ['m1', 'm9']);
      final parsed = Message.fromJson(unknown);
      expect(parsed.kind, MessageKind.other);
      expect(parsed.body, 'こんにちは');
    });

    test('契約にない種類だけが「その他」になる', () {
      for (final kind in MessageKind.values.where(
        (e) => e != MessageKind.other,
      )) {
        expect(MessageKind.fromWire(kind.wire), kind);
      }
      expect(MessageKind.fromWire(''), MessageKind.other);
    });

    test('最後のメッセージが知らない種類でも、種類「その他」として持つ', () {
      final thread = MessageThread.fromJson({
        'thread_id': 't1',
        'student_id': 's1',
        'coach_id': 'c1',
        'opened_at': '2026-10-26T02:00:00Z',
        'last_message': unknown,
        'unread_count': 1,
      });
      expect(thread.lastMessage?.kind, MessageKind.other);
      expect(thread.lastMessage?.body, 'こんにちは');
      expect(thread.unreadCount, 1);
    });
  });

  test('MessageThreadList.fromJson のスレッドは書き換えられない', () {
    final list = MessageThreadList.fromJson({
      'threads': [
        {
          'thread_id': 't1',
          'student_id': 's1',
          'coach_id': 'c1',
          'opened_at': '2026-10-26T02:00:00Z',
          'last_message': message,
          'unread_count': 1,
        },
      ],
    });

    expect(list.threads, hasLength(1));
    expect(() => list.threads.clear(), throwsUnsupportedError);
    expect(() => list.threads[0] = list.threads[0], throwsUnsupportedError);
  });

  test('MessageList.fromJson のメッセージは書き換えられない', () {
    final list = MessageList.fromJson({
      'thread_id': 't1',
      'messages': [message],
    });

    expect(list.messages, hasLength(1));
    expect(() => list.messages.add(list.messages[0]), throwsUnsupportedError);
    expect(() => list.messages[0] = list.messages[0], throwsUnsupportedError);
  });

  group('VideoReview.fromJson', () {
    final review = {
      'review_id': null,
      'video_id': 'v1',
      'student_id': 's1',
      'coach_id': 'c1',
      'status': 'in_progress',
      'comment': null,
      'returned_at': null,
    };

    test('AI の一次フィードバックを読む', () {
      final parsed = VideoReview.fromJson({
        ...review,
        'ai_feedback': {
          'comment': 'フォームは安定しています',
          'created_at': '2026-10-26T03:00:00Z',
        },
      });
      expect(parsed.aiFeedback?.comment, 'フォームは安定しています');
      expect(parsed.aiFeedback?.createdAt, DateTime.utc(2026, 10, 26, 3));
      expect(parsed.comment, isNull);
    });

    test('AI の一次フィードバックが null・項目がなければ null', () {
      expect(
        VideoReview.fromJson({...review, 'ai_feedback': null}).aiFeedback,
        isNull,
      );
      expect(VideoReview.fromJson(review).aiFeedback, isNull);
    });

    test('動画を送った時点のレッスンメニューを読み、null・項目がなければ null', () {
      final parsed = VideoReview.fromJson({
        ...review,
        'lesson_menu': {'lesson_menu_id': 'm1', 'name': 'サーブの動画添削'},
      });
      expect(parsed.lessonMenu?.lessonMenuId, 'm1');
      expect(parsed.lessonMenu?.name, 'サーブの動画添削');
      expect(
        VideoReview.fromJson({...review, 'lesson_menu': null}).lessonMenu,
        isNull,
      );
      expect(VideoReview.fromJson(review).lessonMenu, isNull);
    });
  });

  group('Video.fromJson', () {
    final video = {
      'video_id': 'v1',
      'student_id': 's1',
      'coach_id': 'c1',
      'sport': 'tennis',
      'status': 'awaiting_upload',
      'upload_url': 'https://objectstore.invalid/upload',
      'created_at': '2026-10-26T03:00:00Z',
    };

    test('動画を送った時点のレッスンメニューを読み、null・項目がなければ null', () {
      final parsed = Video.fromJson({
        ...video,
        'lesson_menu': {'lesson_menu_id': 'm1', 'name': 'サーブの動画添削'},
      });
      expect(parsed.lessonMenu?.lessonMenuId, 'm1');
      expect(parsed.lessonMenu?.name, 'サーブの動画添削');
      expect(
        Video.fromJson({...video, 'lesson_menu': null}).lessonMenu,
        isNull,
      );
      expect(Video.fromJson(video).lessonMenu, isNull);
    });
  });

  test('SubmitVideoRequest はメニューを選んだときだけ lesson_menu_id を書く', () {
    const request = SubmitVideoRequest(
      coachId: 'c1',
      sport: Sport.tennis,
      contentType: VideoContentType.mp4,
    );
    expect(request.toJson(), isNot(contains('lesson_menu_id')));
    expect(
      const SubmitVideoRequest(
        coachId: 'c1',
        sport: Sport.tennis,
        contentType: VideoContentType.mp4,
        lessonMenuId: 'm1',
      ).toJson()['lesson_menu_id'],
      'm1',
    );
  });


  group('CoachReview.fromJson', () {
    final review = {
      'review_id': 'r1',
      'lesson_id': 'l1',
      'rating': 5,
      'comment': '分かりやすかった',
      'submitted_at': '2026-10-26T03:00:00Z',
    };

    test('評価したレッスンのレッスンメニューを読み、null・項目がなければ null', () {
      final parsed = CoachReview.fromJson({
        ...review,
        'lesson_menu': {'lesson_menu_id': 'm1', 'name': 'サーブ集中レッスン'},
      });
      expect(parsed.lessonMenu?.lessonMenuId, 'm1');
      expect(parsed.lessonMenu?.name, 'サーブ集中レッスン');
      expect(
        CoachReview.fromJson({...review, 'lesson_menu': null}).lessonMenu,
        isNull,
      );
      expect(CoachReview.fromJson(review).lessonMenu, isNull);
    });
  });

  group('Slot.fromJson', () {
    final slot = {
      'slot_id': 's1',
      'coach_id': 'c1',
      'sport': 'tennis',
      'starts_at': '2026-11-01T10:00:00Z',
      'ends_at': '2026-11-01T11:00:00Z',
      'capacity': 1,
      'remaining_seats': 1,
    };

    test('コーチの表示名と自己紹介文を読む', () {
      final parsed = Slot.fromJson({
        ...slot,
        'coach_display_name': 'たなか',
        'coach_bio': 'テニス歴 20 年',
      });
      expect(parsed.coachDisplayName, 'たなか');
      expect(parsed.coachBio, 'テニス歴 20 年');
    });

    test('コーチの紹介の項目がなければ null', () {
      final parsed = Slot.fromJson(slot);
      expect(parsed.slotId, 's1');
      expect(parsed.coachDisplayName, isNull);
      expect(parsed.coachBio, isNull);
    });

    test('結びついたレッスンメニューの ID・名前・1 回の時間を読む', () {
      final parsed = Slot.fromJson({
        ...slot,
        'lesson_menu': {
          'lesson_menu_id': 'm1',
          'name': 'フォアハンドの基礎',
          'duration_minutes': 60,
        },
      });
      final menu = parsed.lessonMenu!;
      expect(menu.lessonMenuId, 'm1');
      expect(menu.name, 'フォアハンドの基礎');
      expect(menu.durationMinutes, 60);
    });

    test('非同期に直されたメニューは 1 回の時間が null', () {
      final parsed = Slot.fromJson({
        ...slot,
        'lesson_menu': {
          'lesson_menu_id': 'm1',
          'name': 'サーブの動画添削',
          'duration_minutes': null,
        },
      });
      expect(parsed.lessonMenu!.name, 'サーブの動画添削');
      expect(parsed.lessonMenu!.durationMinutes, isNull);
    });

    test('メニューが null か載っていなければ null', () {
      expect(Slot.fromJson({...slot, 'lesson_menu': null}).lessonMenu, isNull);
      expect(Slot.fromJson(slot).lessonMenu, isNull);
    });
  });

  test('MyBookingList.fromJson は予約とスロットを読み、一覧は書き換えられない', () {
    final list = MyBookingList.fromJson({
      'bookings': [
        {
          'booking': {
            'booking_id': 'b1',
            'slot_id': 's1',
            'student_id': 'st1',
            'coach_id': 'c1',
            'is_trial': true,
            'status': 'cancelled',
            'starts_at': '2026-11-01T10:00:00Z',
            'created_at': '2026-10-26T00:00:00Z',
            'confirmed_at': null,
            'cancelled_at': '2026-10-27T00:00:00Z',
          },
          'slot': {
            'slot_id': 's1',
            'coach_id': 'c1',
            'sport': 'tennis',
            'starts_at': '2026-11-01T10:00:00Z',
            'ends_at': '2026-11-01T11:00:00Z',
            'capacity': 1,
            'remaining_seats': 1,
            'coach_display_name': 'たなか',
          },
        },
      ],
    });

    expect(list.bookings, hasLength(1));
    final item = list.bookings[0];
    expect(item.booking.bookingId, 'b1');
    expect(item.booking.status, BookingStatus.cancelled);
    expect(item.slot.sport, Sport.tennis);
    expect(item.slot.endsAt, DateTime.utc(2026, 11, 1, 11));
    expect(item.slot.coachDisplayName, 'たなか');
    expect(item.slot.coachBio, isNull);
    expect(item.booking.lessonMenu, isNull);
    expect(() => list.bookings.clear(), throwsUnsupportedError);
  });

  test('MyVideoList をコンストラクタで直接作っても、渡した元を変えても一覧は変わらず、書き換えられない', () {
    final video = MyVideo(
      videoId: 'v1',
      coachId: 'c1',
      sport: Sport.tennis,
      submittedAt: DateTime.utc(2026, 10, 26),
      reviewStatus: VideoReviewStatus.inProgress,
    );
    final source = [video];
    final list = MyVideoList(videos: source);
    source.clear();
    expect(list.videos, [video]);
    expect(() => list.videos.clear(), throwsUnsupportedError);
    expect(() => list.videos.add(video), throwsUnsupportedError);
  });

  test('MyVideoList.fromJson は添削の状態と任意の項目を読み、一覧は書き換えられない', () {
    final list = MyVideoList.fromJson({
      'videos': [
        {
          'video_id': 'v1',
          'coach_id': 'c1',
          'coach_display_name': 'たなか',
          'sport': 'tennis',
          'submitted_at': '2026-10-26T00:00:00Z',
          'review_status': 'in_progress',
          'returned_at': null,
          'lesson_menu': {'lesson_menu_id': 'm1', 'name': 'サーブの動画添削'},
        },
        {
          'video_id': 'v2',
          'coach_id': 'c2',
          'sport': 'soccer',
          'submitted_at': '2026-10-25T00:00:00Z',
          'review_status': 'returned',
          'returned_at': '2026-10-25T12:00:00Z',
          'lesson_menu': null,
        },
      ],
    });

    expect(list.videos, hasLength(2));
    final waiting = list.videos[0];
    expect(waiting.videoId, 'v1');
    expect(waiting.coachDisplayName, 'たなか');
    expect(waiting.sport, Sport.tennis);
    expect(waiting.submittedAt, DateTime.utc(2026, 10, 26));
    expect(waiting.reviewStatus, VideoReviewStatus.inProgress);
    expect(waiting.returnedAt, isNull);
    expect(waiting.lessonMenu?.name, 'サーブの動画添削');
    final returned = list.videos[1];
    // コーチのアカウントが引けないときは表示名が載らない
    expect(returned.coachDisplayName, isNull);
    expect(returned.reviewStatus, VideoReviewStatus.returned);
    expect(returned.returnedAt, DateTime.utc(2026, 10, 25, 12));
    expect(returned.lessonMenu, isNull);
    expect(() => list.videos.clear(), throwsUnsupportedError);
  });

  group('Booking のレッスンメニュー', () {
    final booking = {
      'booking_id': 'b1',
      'slot_id': 's1',
      'student_id': 'st1',
      'coach_id': 'c1',
      'is_trial': false,
      'status': 'confirmed',
      'starts_at': '2026-11-01T10:00:00Z',
      'created_at': '2026-10-26T00:00:00Z',
      'confirmed_at': '2026-10-26T00:01:00Z',
      'cancelled_at': null,
    };

    test('予約した時点のメニューの ID・名前・1 回の時間を読む', () {
      final menu = Booking.fromJson({
        ...booking,
        'lesson_menu': {
          'lesson_menu_id': 'm1',
          'name': 'フォアハンドの基礎',
          'duration_minutes': 60,
        },
      }).lessonMenu!;
      expect(menu.lessonMenuId, 'm1');
      expect(menu.name, 'フォアハンドの基礎');
      expect(menu.durationMinutes, 60);
    });

    test('予約した時点で非同期に直されていたメニューは 1 回の時間が null', () {
      final menu = Booking.fromJson({
        ...booking,
        'lesson_menu': {
          'lesson_menu_id': 'm1',
          'name': 'サーブの動画添削',
          'duration_minutes': null,
        },
      }).lessonMenu!;
      expect(menu.name, 'サーブの動画添削');
      expect(menu.durationMinutes, isNull);
    });

    test('メニューが null か載っていなければ null', () {
      expect(
        Booking.fromJson({...booking, 'lesson_menu': null}).lessonMenu,
        isNull,
      );
      expect(Booking.fromJson(booking).lessonMenu, isNull);
    });
  });

  group('契約プラン', () {
    test('StartSubscriptionRequest は plan を選んだときだけ送る', () {
      expect(
        const StartSubscriptionRequest(
          sport: Sport.tennis,
          paymentMethodToken: 'tok_mock',
        ).toJson(),
        {'sport': 'tennis', 'payment_method_token': 'tok_mock'},
      );
      expect(
        const StartSubscriptionRequest(
          sport: Sport.tennis,
          plan: SubscriptionPlan.plus,
          paymentMethodToken: 'tok_mock',
        ).toJson(),
        {
          'sport': 'tennis',
          'plan': 'plus',
          'payment_method_token': 'tok_mock',
        },
      );
    });

    test('Entitlement.fromJson はプランと暦月の上限を読み、ワンショットのプランは null', () {
      final plus = Entitlement.fromJson({
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
      });
      expect(plus.plan, SubscriptionPlan.plus);
      expect(plus.liveLessonQuota!.limit, 4);

      final oneShot = Entitlement.fromJson({
        'entitlement_id': 'e2',
        'student_id': 's2',
        'contract_type': 'one_shot',
        'sport': 'golf',
        'plan': null,
        'live_lesson_quota': null,
        'remaining_video_reviews': 1,
      });
      expect(oneShot.plan, isNull);
    });

    test('Subscription.fromJson はプランを読む', () {
      final subscription = Subscription.fromJson({
        'subscription_id': 'sub1',
        'student_id': 's1',
        'sport': 'tennis',
        'plan': 'standard',
        'status': 'active',
        'started_at': '2026-10-26T00:00:00Z',
      });
      expect(subscription.plan, SubscriptionPlan.standard);
    });
  });

  test('SubscriptionPlanList をコンストラクタで直接作っても、渡した元を変えても一覧は変わらず、書き換えられない', () {
    const standard = SubscriptionPlanDetail(
      plan: SubscriptionPlan.standard,
      liveLessonsPerMonth: 2,
    );
    final source = [standard];
    final list = SubscriptionPlanList(plans: source);
    source.clear();
    expect(list.plans, [standard]);
    expect(() => list.plans.clear(), throwsUnsupportedError);
    expect(() => list.plans.add(standard), throwsUnsupportedError);
  });

  test('SubscriptionPlanList.fromJson はプランと上限を読み、一覧は書き換えられない', () {
    final list = SubscriptionPlanList.fromJson({
      'plans': [
        {'plan': 'standard', 'live_lessons_per_month': 2},
        {'plan': 'plus', 'live_lessons_per_month': 4},
      ],
    });
    expect(list.plans.map((p) => p.plan), [
      SubscriptionPlan.standard,
      SubscriptionPlan.plus,
    ]);
    expect(list.plans.map((p) => p.liveLessonsPerMonth), [2, 4]);
    expect(() => list.plans.clear(), throwsUnsupportedError);
  });

  test('CoachListingList をコンストラクタで直接作っても、渡した元を変えても一覧は変わらず、書き換えられない', () {
    const coach = CoachListing(
      coachId: 'c1',
      displayName: '佐藤',
      sport: Sport.tennis,
      reviewCount: 0,
      averageRating: null,
      publishedLessonMenuCount: 0,
    );
    final source = [coach];
    final list = CoachListingList(coaches: source);
    source.clear();
    expect(list.coaches, [coach]);
    expect(() => list.coaches.clear(), throwsUnsupportedError);
    expect(() => list.coaches.add(coach), throwsUnsupportedError);
  });

  test('CoachListingList.fromJson の一覧は書き換えられない', () {
    final list = CoachListingList.fromJson({
      'coaches': [
        {
          'coach_id': 'c1',
          'display_name': '佐藤',
          'sport': 'tennis',
          'review_count': 0,
          'average_rating': null,
          'published_lesson_menu_count': 0,
        },
      ],
    });
    expect(list.coaches.single.coachId, 'c1');
    expect(() => list.coaches.clear(), throwsUnsupportedError);
  });

  test('SubscriptionPlanDetail は上限が 1 未満なら作れない（契約の minimum: 1）', () {
    for (final value in [0, -1]) {
      expect(
        () => SubscriptionPlanDetail.fromJson({
          'plan': 'plus',
          'live_lessons_per_month': value,
        }),
        throwsFormatException,
      );
      expect(
        () => SubscriptionPlanDetail(
          plan: SubscriptionPlan.plus,
          liveLessonsPerMonth: value,
        ),
        throwsA(isA<AssertionError>()),
      );
    }
  });
}
