import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/booking_pending_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

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

  testWidgets('通話か動画の送付を選ぶと、それぞれの画面に進む', (tester) async {
    final confirmed = pending().copyWith(booking: Fixtures.bookingConfirmed);

    final container = await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: confirmed,
    );
    await tapText(tester, 'ビデオ通話で受ける');
    expect(container.read(studentFlowProvider).step, FlowStep.liveLesson);

    final other = await pumpScreen(
      tester,
      const BookingPendingScreen(),
      api: FakeStudentApi(),
      initial: confirmed,
    );
    await tapText(tester, '動画を送って添削を受ける');
    expect(other.read(studentFlowProvider).step, FlowStep.videoSubmit);
  });
}
