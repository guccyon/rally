import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/live_lesson_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  testWidgets('通話を開始・終了すると、完了記録に進む（映像はモック）', (tester) async {
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

    expect(find.text('映像はモックです'), findsOneWidget);
    await tapText(tester, '通話を開始する');

    final request = api.requests.single as StartLiveLessonSessionRequest;
    expect(request.toJson(), {'booking_id': Fixtures.bookingId});
    expect(find.text('通話中（映像はモック）'), findsOneWidget);

    await tapText(tester, '通話を終了する');

    expect(api.calls, ['startLiveLessonSession', 'endLiveLessonSession']);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.lessonCompletion);
    expect(state.session!.status, LiveLessonSessionStatus.ended);
  });
}
