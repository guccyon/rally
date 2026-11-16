import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/live_lesson_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  testWidgets('通話を開始・終了すると、完了記録に進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const LiveLessonScreen(),
      api: api,
      initial: StudentFlowState(
        step: FlowStep.liveLesson,
        student: Fixtures.student,
        bookedSlot: Fixtures.slot,
        booking: Fixtures.bookingConfirmed,
      ),
    );

    expect(find.text('「通話を開始する」を押すと、コーチと通話できます'), findsOneWidget);
    await tapText(tester, '通話を開始する');

    final request = api.requests.single as StartLiveLessonSessionRequest;
    expect(request.toJson(), {'booking_id': Fixtures.bookingId});
    expect(find.text('コーチと通話中です\n映像が表示されなくても、故障ではありません'), findsOneWidget);
    expect(find.textContaining('モック'), findsNothing);

    await tapText(tester, '通話を終了する');

    expect(api.calls, ['startLiveLessonSession', 'endLiveLessonSession']);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.lessonCompletion);
    expect(state.session!.status, LiveLessonSessionStatus.ended);
  });

  const waitingText = 'コーチを待っています\nコーチが入室すると、通話が始まります';
  const inCallText = 'コーチと通話中です\n映像が表示されなくても、故障ではありません';

  Future<ProviderContainer> pumpLiveLesson(
    WidgetTester tester,
    FakeStudentApi api,
  ) => pumpScreen(
    tester,
    const LiveLessonScreen(),
    api: api,
    initial: StudentFlowState(
      step: FlowStep.liveLesson,
      student: Fixtures.student,
      bookedSlot: Fixtures.slot,
      booking: Fixtures.bookingConfirmed,
    ),
  );

  testWidgets('コーチの入室前は待機を出し、取り直しでコーチの入室を知ると通話中に変わる', (tester) async {
    final api = FakeStudentApi()..liveSession = Fixtures.sessionAwaitingCoach;
    await pumpLiveLesson(tester, api);

    await tapText(tester, '通話を開始する');
    expect(find.text(waitingText), findsOneWidget);
    expect(find.text(inCallText), findsNothing);
    expect(find.text('通話を終了する'), findsOneWidget);

    // 間隔より前には取り直さない
    await tester.pump(
      liveLessonAwaitingCoachRefetchInterval - const Duration(seconds: 1),
    );
    expect(api.calls, ['startLiveLessonSession']);

    // コーチがまだ入室していなければ待機のまま、次の間隔でまた取り直す
    await tester.pump(const Duration(seconds: 1));
    expect(api.calls, ['startLiveLessonSession', 'getLiveLessonSession']);
    expect(find.text(waitingText), findsOneWidget);

    api.liveSession = Fixtures.sessionInProgress;
    await tester.pump(liveLessonAwaitingCoachRefetchInterval);
    await tester.pump();
    expect(find.text(inCallText), findsOneWidget);
    expect(find.text(waitingText), findsNothing);

    // 通話中になったら取り直さない
    await tester.pump(liveLessonAwaitingCoachRefetchInterval * 3);
    expect(api.calls.where((c) => c == 'getLiveLessonSession'), hasLength(2));
  });

  testWidgets('コーチが先に入室していれば、最初から通話中で取り直さない', (tester) async {
    final api = FakeStudentApi();
    await pumpLiveLesson(tester, api);

    await tapText(tester, '通話を開始する');
    expect(find.text(inCallText), findsOneWidget);
    expect(find.text(waitingText), findsNothing);

    await tester.pump(liveLessonAwaitingCoachRefetchInterval * 3);
    expect(api.calls, ['startLiveLessonSession']);
  });

  testWidgets('取り直しに失敗しても文言は出さず、待機のまま次の間隔で取り直す', (tester) async {
    final api = FakeStudentApi(
      failures: {'getLiveLessonSession': const StudentApiException()},
    )..liveSession = Fixtures.sessionAwaitingCoach;
    final container = await pumpLiveLesson(tester, api);

    await tapText(tester, '通話を開始する');
    await tester.pump(liveLessonAwaitingCoachRefetchInterval);
    await tester.pump(liveLessonAwaitingCoachRefetchInterval);
    expect(api.calls.where((c) => c == 'getLiveLessonSession'), hasLength(2));
    expect(find.text(waitingText), findsOneWidget);
    expect(container.read(studentFlowProvider).errorMessage, isNull);
  });

  testWidgets('待機のまま通話を終了したら、取り直しをやめる', (tester) async {
    final api = FakeStudentApi()..liveSession = Fixtures.sessionAwaitingCoach;
    final container = await pumpLiveLesson(tester, api);

    await tapText(tester, '通話を開始する');
    await tapText(tester, '通話を終了する');
    expect(container.read(studentFlowProvider).step, FlowStep.lessonCompletion);

    await tester.pump(liveLessonAwaitingCoachRefetchInterval * 3);
    expect(api.calls, ['startLiveLessonSession', 'endLiveLessonSession']);
  });
}
