import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/booking_pending_screen.dart';
import 'package:rally_student_app/state/prejoin_check.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';

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

  test('入室前チェックを開けるのは開始 5 分前ちょうどから、入室できるのは開始時刻ちょうどから', () {
    final startsAt = Fixtures.slot.startsAt;
    const second = Duration(seconds: 1);
    expect(prejoinCheckLeadTime, const Duration(minutes: 5));
    final opensAt = startsAt.subtract(prejoinCheckLeadTime);
    expect(canOpenPrejoinCheck(startsAt, opensAt.subtract(second)), isFalse);
    expect(canOpenPrejoinCheck(startsAt, opensAt), isTrue);
    expect(canJoinLiveLesson(startsAt, startsAt.subtract(second)), isFalse);
    expect(canJoinLiveLesson(startsAt, startsAt), isTrue);
  });
}
