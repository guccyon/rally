import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/booking_pending_screen.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/prejoin_check.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';
import 'package:rally_student_app/ui/labels.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  StudentFlowState pending() => StudentFlowState(
    step: FlowStep.bookingPending,
    student: Fixtures.student,
    bookedSlot: Fixtures.slot,
    booking: Fixtures.bookingPending,
  );

  testWidgets('確定待ちの予約を確定すると、初回レッスンの受け方を選べる', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: api,
      initial: pending(),
    );

    expect(find.text('予約の確定待ち'), findsOneWidget);
    expect(find.text('状態: 確定待ち'), findsOneWidget);
    expect(find.text('ビデオ通話で受ける'), findsNothing);

    await tapText(tester, '予約を確定する');

    expect(api.calls, ['confirmBooking']);
    expect(api.requests.single, Fixtures.bookingId);
    expect(find.text('予約が確定しました'), findsOneWidget);
    expect(find.text('ビデオ通話で受ける'), findsOneWidget);
    expect(find.text('動画を送って添削を受ける'), findsOneWidget);
  });

  testWidgets('plus（上限 4 回）の枠を使い切って確定を断られたら、固定の回数を出さずに枠を使い切ったと出す', (
    tester,
  ) async {
    final api = FakeStudentApi(
      failures: {
        'confirmBooking': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.liveLessonQuotaExceeded,
            message: 'quota exceeded',
          ),
        ),
      },
    );
    final container = await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: api,
      initial: pending(),
      subscription: SubscriptionState(
        entitlement: Fixtures.subscriptionStartedWith(SubscriptionPlan.plus)
            .entitlement,
      ),
    );

    await tapText(tester, '予約を確定する');

    expect(find.text('今月のビデオ通話の枠を使い切っています。'), findsOneWidget);
    expect(find.textContaining('2 回'), findsNothing);
    expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);
  });

  testWidgets('予約した時点のメニューの写しがあれば、確定の前後にメニューの名前と 1 回の時間を出す', (tester) async {
    // 予約した後に枠のメニューが直されても、出すのは予約の写し
    final initial = StudentFlowState(
      step: FlowStep.bookingPending,
      student: Fixtures.student,
      bookedSlot: Fixtures.slotWithCoach(
        lessonMenu: SlotLessonMenu(
          lessonMenuId: Fixtures.slotLessonMenu.lessonMenuId,
          name: 'フォアハンドの応用',
          durationMinutes: 90,
        ),
      ),
      booking: Fixtures.withLessonMenu(
        Fixtures.bookingPending,
        Fixtures.bookingLessonMenu,
      ),
    );
    await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(bookingLessonMenu: Fixtures.bookingLessonMenu),
      initial: initial,
    );

    const label = 'メニュー: フォアハンドの基礎（1 回 60 分）';
    expect(find.text(label), findsOneWidget);
    expect(find.textContaining('フォアハンドの応用'), findsNothing);
    expect(
      tester.getTopLeft(find.text(label)).dy,
      greaterThan(tester.getTopLeft(find.textContaining('開始: ')).dy),
    );

    await tapText(tester, '予約を確定する');

    expect(find.text('予約が確定しました'), findsOneWidget);
    expect(find.text(label), findsOneWidget);
  });

  testWidgets('予約にメニューの写しがなければ、枠にメニューがあっても出さない', (tester) async {
    await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: pending().copyWith(
        bookedSlot: Fixtures.slotWithCoach(lessonMenu: Fixtures.slotLessonMenu),
      ),
    );

    expect(find.textContaining('メニュー'), findsNothing);

    await tapText(tester, '予約を確定する');

    expect(find.text('予約が確定しました'), findsOneWidget);
    expect(find.textContaining('メニュー'), findsNothing);
  });

  testWidgets('メニューのない枠の予約には、今までどおりメニューを出さない', (tester) async {
    await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: pending(),
    );

    expect(find.textContaining('メニュー'), findsNothing);
  });

  testWidgets('開始 5 分前を過ぎて通話か動画の送付を選ぶと、入室前チェックか動画の送付に進む', (tester) async {
    final confirmed = pending().copyWith(booking: Fixtures.bookingConfirmed);

    final container = await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: confirmed,
      now: prejoinCheckOpensAt(Fixtures.slot.startsAt),
    );
    expect(find.textContaining('から入室前チェックができます'), findsNothing);
    await tapText(tester, 'ビデオ通話で受ける');
    expect(container.read(studentFlowProvider).step, FlowStep.prejoinCheck);

    final other = await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: confirmed,
    );
    await tapText(tester, '動画を送って添削を受ける');
    expect(other.read(studentFlowProvider).step, FlowStep.videoSubmit);
  });

  testWidgets('開始 5 分前より早いと「ビデオ通話で受ける」は押せず開ける時刻を出し、開いたまま 5 分前になると押せる', (
    tester,
  ) async {
    final opensAt = prejoinCheckOpensAt(Fixtures.slot.startsAt);
    var now = opensAt.subtract(const Duration(seconds: 2));
    final container = await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: pending().copyWith(booking: Fixtures.bookingConfirmed),
      clock: () => now,
    );

    final button = find.widgetWithText(FilledButton, 'ビデオ通話で受ける');
    expect(tester.widget<FilledButton>(button).onPressed, isNull);
    final local = opensAt.toLocal();
    final clock = '${local.hour}:${local.minute.toString().padLeft(2, '0')}';
    expect(
      find.text(
        '$clock から入室前チェックができます（開始 5 分前から）。'
        'カメラとマイクの映りと音を確かめてから、開始時刻に入室します。',
      ),
      findsOneWidget,
    );
    await tester.tap(button);
    await tester.pump();
    expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);

    now = opensAt;
    await tester.pump(const Duration(seconds: 1));
    expect(tester.widget<FilledButton>(button).onPressed, isNotNull);
    expect(find.textContaining('から入室前チェックができます'), findsNothing);
  });

  group('予約の ID でサーバーから読み直して開く', () {
    final home = StudentFlowState(
      step: FlowStep.home,
      accessToken: Fixtures.registered.accessToken,
      student: Fixtures.student,
    );

    /// 自分の予約の一覧の [Fixtures.bookingId] の予約（[status] の状態。体験予約ではない）
    MyBooking mine(BookingStatus status) {
      final item = Fixtures.myBooking(
        bookingId: Fixtures.bookingId,
        startsIn: const Duration(days: 2),
        status: status,
        coachDisplayName: '山田 花子',
      );
      return MyBooking(
        booking: Fixtures.bookingWith(
          Fixtures.withLessonMenu(item.booking, null),
          status: status,
        ),
        slot: item.slot,
      );
    }

    Future<ProviderContainer> openById(
      WidgetTester tester,
      FakeStudentApi api,
    ) async {
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: home,
      );
      await container
          .read(studentFlowProvider.notifier)
          .openBooking(Fixtures.bookingId);
      await tester.pumpAndSettle();
      return container;
    }

    testWidgets('確定待ちなら中身と「予約を確定する」「この予約をキャンセルする」を出す', (tester) async {
      final api = FakeStudentApi(myBookings: [mine(BookingStatus.pending)]);
      final container = await openById(tester, api);

      expect(api.calls, ['listMyBookings']);
      expect(find.text('予約の確定待ち'), findsOneWidget);
      expect(find.text('状態: 確定待ち'), findsOneWidget);
      expect(find.text('予約を確定する'), findsOneWidget);
      expect(find.text('この予約をキャンセルする'), findsOneWidget);
      expect(
        container.read(studentFlowProvider).bookedSlot?.slotId,
        'slot-${Fixtures.bookingId}',
      );
    });

    testWidgets('開いたら読み直しのあいだ読み込み中にし、読めたら中身を出す', (tester) async {
      final api = FakeStudentApi(myBookings: [mine(BookingStatus.pending)]);
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        // 前に作った別の予約をアプリの段に持っている
        initial: home.copyWith(booking: Fixtures.bookingConfirmed),
      );
      final opening = container
          .read(studentFlowProvider.notifier)
          .openBooking(Fixtures.bookingId);
      expect(
        container.read(studentFlowProvider).bookingLookup,
        BookingLookup.loading,
      );
      await opening;
      await tester.pumpAndSettle();
      expect(find.text('予約の確定待ち'), findsOneWidget);
    });

    testWidgets('読み込み中は「予約を読み込んでいます…」だけを出し、前の予約を出さない', (tester) async {
      await pumpScreen(
        tester,
        const BookingPendingScreen(),
        api: FakeStudentApi(),
        initial: home.copyWith(
          step: FlowStep.bookingPending,
          booking: Fixtures.bookingConfirmed,
          bookingLookup: BookingLookup.loading,
        ),
      );

      expect(find.text('予約を読み込んでいます…'), findsOneWidget);
      expect(find.text('予約が確定しました'), findsNothing);
    });

    testWidgets('もう確定済みなら「予約が確定しました」に切り替える', (tester) async {
      await openById(
        tester,
        FakeStudentApi(myBookings: [mine(BookingStatus.confirmed)]),
      );

      expect(find.text('予約が確定しました'), findsOneWidget);
      expect(find.text('予約を確定する'), findsNothing);
      expect(find.text('ビデオ通話で受ける'), findsOneWidget);
    });

    testWidgets('キャンセル済み・見つからないなら旨と「ホームに戻る」を出し、確定もキャンセルも出さない', (tester) async {
      for (final bookings in [
        [mine(BookingStatus.cancelled)],
        <MyBooking>[],
      ]) {
        final api = FakeStudentApi(myBookings: bookings);
        final container = await openById(tester, api);

        expect(
          find.text('この予約はもう確定待ちではありません。ホームで次にやることを確かめてください。'),
          findsOneWidget,
        );
        expect(find.text('予約を確定する'), findsNothing);
        expect(find.text('この予約をキャンセルする'), findsNothing);

        await tapText(tester, 'ホームに戻る');
        expect(container.read(studentFlowProvider).step, FlowStep.home);
      }
    });

    testWidgets('読めなければ旨と「ホームに戻る」を出す（失敗の文言は出さない）', (tester) async {
      final api = FakeStudentApi(
        failures: {'listMyBookings': Fixtures.unreachable},
      );
      final container = await openById(tester, api);

      expect(find.text('予約を読み込めませんでした。「ホームに戻る」から、もう一度開けます。'), findsOneWidget);
      expect(container.read(studentFlowProvider).errorMessage, isNull);
      expect(find.text('ホームに戻る'), findsOneWidget);
    });

    testWidgets('確定が 409 invalid_booking_state なら読み直して、今の状態を出す', (
      tester,
    ) async {
      final api = FakeStudentApi(
        myBookings: [mine(BookingStatus.pending)],
        failures: {
          'confirmBooking': const StudentApiException(
            statusCode: 409,
            error: ApiError(
              code: ErrorCode.invalidBookingState,
              message: 'invalid booking state',
            ),
          ),
        },
      );
      final container = await openById(tester, api);
      // ほかの端末で確定した
      api.myBookings[0] = mine(BookingStatus.confirmed);
      api.calls.clear();

      await tapText(tester, '予約を確定する');

      expect(api.calls, ['confirmBooking', 'listMyBookings']);
      expect(container.read(studentFlowProvider).errorMessage, isNull);
      expect(find.text('予約が確定しました'), findsOneWidget);
    });

    testWidgets('「この予約をキャンセルする」で確認し、「キャンセルする」で枠を返してホームへ戻る', (tester) async {
      final api = FakeStudentApi(myBookings: [mine(BookingStatus.pending)]);
      final container = await openById(tester, api);
      api.calls.clear();

      await tapText(tester, 'この予約をキャンセルする');
      expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
      final slot = api.myBookings.single.slot;
      expect(
        find.text(
          '${formatTimeRange(slot.startsAt, slot.endsAt)} のテニスの予約をキャンセルします。'
          '押さえていたスロットはほかの生徒も予約できるようになります。',
        ),
        findsOneWidget,
      );

      // やめると何も送らない
      await tapText(tester, 'やめる');
      expect(api.calls, isEmpty);
      expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);

      await tapText(tester, 'この予約をキャンセルする');
      await tapText(tester, 'キャンセルする');

      expect(api.calls.first, 'cancelBooking');
      expect(api.requests.first, Fixtures.bookingId);
      expect(api.myBookings.single.booking.status, BookingStatus.cancelled);
      expect(container.read(studentFlowProvider).step, FlowStep.home);
      expect(api.calls, contains('getMyNextActions'));
      expect(find.text('予約をキャンセルしました'), findsOneWidget);
    });

    testWidgets('キャンセルに失敗したら予約の画面に留まり、失敗の文言を出す', (tester) async {
      final api = FakeStudentApi(
        myBookings: [mine(BookingStatus.pending)],
        failures: {'cancelBooking': Fixtures.unreachable},
      );
      final container = await openById(tester, api);

      await tapText(tester, 'この予約をキャンセルする');
      await tapText(tester, 'キャンセルする');

      expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);
      expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
      expect(find.text('予約をキャンセルしました'), findsNothing);
    });
  });

  group('予約をサーバーの記録から読み直す（#1514）', () {
    /// 確定待ちの体験予約がサーバーにもある、ホームから体験予約した直後の状態（アプリ全体の画面で出す）
    StudentFlowState justBooked() =>
        pending().copyWith(accessToken: Fixtures.registered.accessToken);

    MyBooking mine(Booking booking) =>
        MyBooking(booking: booking, slot: Fixtures.slot);

    Booking cancelledTrial() => Fixtures.bookingWith(
      Fixtures.bookingPending,
      status: BookingStatus.cancelled,
      cancelledAt: Fixtures.now,
    );

    const goneNotice = 'この予約はもう確定待ちではありません。ホームで次にやることを確かめてください。';

    testWidgets('確定待ちの予約を自分の予約でキャンセルして戻ると、キャンセル済みと出て「予約を確定する」は出ない', (
      tester,
    ) async {
      final api = FakeStudentApi(myBookings: [mine(Fixtures.bookingPending)]);
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: justBooked(),
      );

      await tester.tap(find.byTooltip('自分の予約'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(OutlinedButton, 'この予約をキャンセルする'));
      await tester.pumpAndSettle();
      await tapText(tester, 'キャンセルする');
      expect(find.text('状態: キャンセル済み'), findsOneWidget);

      await tester.tap(find.byTooltip('自分の予約を閉じる'));
      await tester.pumpAndSettle();

      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.bookingPending);
      expect(state.booking!.status, BookingStatus.cancelled);
      expect(api.calls.last, 'listMyBookings');
      expect(find.text('状態: キャンセル済み'), findsOneWidget);
      expect(find.text(goneNotice), findsOneWidget);
      expect(find.text('予約の確定待ち'), findsNothing);
      expect(find.text('予約を確定する'), findsNothing);
      expect(find.text('この予約をキャンセルする'), findsNothing);
      expect(find.text('ビデオ通話で受ける'), findsNothing);
      expect(find.text('動画を送って添削を受ける'), findsNothing);

      await tapText(tester, 'ホームに戻る');
      expect(container.read(studentFlowProvider).step, FlowStep.home);
    });

    testWidgets('脇の画面にいるあいだにほかの端末で確定していたら、戻ったときに確定済みの表示にする', (tester) async {
      final api = FakeStudentApi(myBookings: [mine(Fixtures.bookingPending)]);
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: justBooked(),
      );

      await tester.tap(find.byTooltip('プロフィール'));
      await tester.pumpAndSettle();
      api.myBookings[0] = mine(Fixtures.bookingConfirmed);
      container.read(studentFlowProvider.notifier).closeProfile();
      await tester.pumpAndSettle();

      expect(find.text('予約が確定しました'), findsOneWidget);
      expect(find.text('予約を確定する'), findsNothing);
      expect(find.text('ビデオ通話で受ける'), findsOneWidget);
    });

    testWidgets(
      '「予約を確定する」が 409 invalid_booking_state なら、予約を読み直してキャンセル済みを出し、ボタンを残さない',
      (tester) async {
        final api = FakeStudentApi(
          myBookings: [mine(cancelledTrial())],
          failures: {
            'confirmBooking': const StudentApiException(
              statusCode: 409,
              error: ApiError(
                code: ErrorCode.invalidBookingState,
                message: '予約の状態が変わっています。予約を確かめてください。',
              ),
            ),
          },
        );
        final container = await pumpScreen(
          tester,
          const BookingPendingScreen(),
          api: api,
          initial: pending(),
        );

        await tapText(tester, '予約を確定する');

        expect(api.calls, ['confirmBooking', 'listMyBookings']);
        expect(
          container.read(studentFlowProvider).booking!.status,
          BookingStatus.cancelled,
        );
        expect(find.text('状態: キャンセル済み'), findsOneWidget);
        expect(find.text(goneNotice), findsOneWidget);
        expect(find.text('予約を確定する'), findsNothing);
        expect(find.text('ホームに戻る'), findsOneWidget);
      },
    );

    testWidgets('自分の予約に見つからない予約は、中身もボタンも出さずに知らせと「ホームに戻る」を出す', (tester) async {
      final container = await pumpScreen(
        tester,
        const BookingPendingScreen(),
        api: FakeStudentApi(myBookings: const []),
        initial: StudentFlowState(student: Fixtures.student),
      );

      await container
          .read(studentFlowProvider.notifier)
          .openBooking(Fixtures.bookingId);
      await tester.pumpAndSettle();

      expect(
        container.read(studentFlowProvider).bookingLookup,
        BookingLookup.notFound,
      );
      expect(find.text(goneNotice), findsOneWidget);
      expect(find.textContaining('開始: '), findsNothing);
      expect(find.text('予約を確定する'), findsNothing);
      expect(find.text('ビデオ通話で受ける'), findsNothing);
      expect(find.text('ホームに戻る'), findsOneWidget);
    });

    testWidgets('予約を読み込めなければ、失敗の知らせと「ホームに戻る」を出す', (tester) async {
      final container = await pumpScreen(
        tester,
        const BookingPendingScreen(),
        api: FakeStudentApi(
          failures: {
            'listMyBookings': const StudentApiException(statusCode: 503),
          },
        ),
        initial: StudentFlowState(student: Fixtures.student),
      );

      await container
          .read(studentFlowProvider.notifier)
          .openBooking(Fixtures.bookingId);
      await tester.pumpAndSettle();

      expect(find.text('予約を読み込めませんでした。「ホームに戻る」から、もう一度開けます。'), findsOneWidget);
      expect(find.text('予約を確定する'), findsNothing);
      expect(find.text('ホームに戻る'), findsOneWidget);
    });

    testWidgets('確定待ちの画面の「この予約をキャンセルする」で確かめてからキャンセルし、ホームに戻る', (tester) async {
      final api = FakeStudentApi(myBookings: [mine(Fixtures.bookingPending)]);
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: justBooked(),
      );

      expect(find.text('予約を確定する'), findsOneWidget);
      await tapText(tester, 'この予約をキャンセルする');
      expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
      expect(
        find.text(
          '${formatTimeRange(Fixtures.slot.startsAt, Fixtures.slot.endsAt)} の'
          'テニスの体験予約をキャンセルします。押さえていたスロットはほかの生徒も予約できるようになります。',
        ),
        findsOneWidget,
      );
      await tapText(tester, 'キャンセルする');

      expect(api.calls.first, 'cancelBooking');
      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.home);
      expect(state.booking!.status, BookingStatus.cancelled);
      expect(find.text('予約をキャンセルしました'), findsOneWidget);
    });

    testWidgets('キャンセル済みの予約からは、動画の送付にも入室前チェックにも進まない（#1364）', (tester) async {
      final container = await pumpScreen(
        tester,
        const BookingPendingScreen(),
        api: FakeStudentApi(),
        initial: pending().copyWith(booking: cancelledTrial()),
        now: prejoinCheckOpensAt(Fixtures.slot.startsAt),
      );

      container.read(studentFlowProvider.notifier).chooseVideo();
      container.read(prejoinCheckProvider.notifier).open();
      await tester.pump();

      expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);
      expect(find.text('動画を送って添削を受ける'), findsNothing);
    });
  });
}
