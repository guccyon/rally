import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/lesson_completion_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final afterCall = StudentFlowState(
    step: FlowStep.lessonCompletion,
    student: Fixtures.student,
    bookedSlot: Fixtures.slot,
    booking: Fixtures.bookingConfirmed,
    session: Fixtures.sessionEnded,
  );

  testWidgets('通話のあとは live_lesson で完了を記録し、初回なら初回と出して月額契約に進める', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const LessonCompletionScreen(),
      api: api,
      initial: afterCall,
    );

    expect(find.text('通話が終わりました。'), findsOneWidget);
    await tapText(tester, '完了を記録する');

    final request = api.requests.single as RecordLessonCompletionRequest;
    expect(request.toJson(), {
      'lesson_type': 'live_lesson',
      'session_id': Fixtures.sessionId,
    });
    expect(find.text('初回レッスンを完了しました'), findsOneWidget);
    expect(find.text('同期レッスン（ビデオ通話）'), findsOneWidget);

    expect(
      find.text(
        '月額契約で、動画とチャットは送り放題、ビデオ通話は月 2 回（スタンダード）・月 4 回（プラス）まで受けられます。',
      ),
      findsOneWidget,
    );
    await tapText(tester, '月額契約へ進む');
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  testWidgets('完了を記録したら評価の画面に進める', (tester) async {
    final container = await pumpScreen(
      tester,
      const LessonCompletionScreen(),
      api: FakeStudentApi(),
      initial: afterCall,
    );

    expect(find.text('レッスンを評価する'), findsNothing);
    await tapText(tester, '完了を記録する');
    await tapText(tester, 'レッスンを評価する');

    expect(container.read(studentFlowProvider).step, FlowStep.lessonReview);
  });

  testWidgets('動画の添削のあとは async_lesson で完了を記録する', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const LessonCompletionScreen(),
      api: api,
      initial: StudentFlowState(
        step: FlowStep.lessonCompletion,
        student: Fixtures.student,
        bookedSlot: Fixtures.slot,
        booking: Fixtures.bookingConfirmed,
        video: Fixtures.video,
        review: Fixtures.reviewReturned,
      ),
    );

    expect(find.text('添削が返されました。'), findsOneWidget);
    await tapText(tester, '完了を記録する');

    final request = api.requests.single as RecordLessonCompletionRequest;
    expect(request.toJson(), {
      'lesson_type': 'async_lesson',
      'video_id': Fixtures.videoId,
    });
    expect(find.text('非同期レッスン（動画添削）'), findsOneWidget);
  });

  testWidgets('記録済みなら文言を出す', (tester) async {
    final api = FakeStudentApi(
      failures: {
        'recordLessonCompletion': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.lessonAlreadyCompleted,
            message: 'lesson already completed',
          ),
        ),
      },
    );
    await pumpScreen(
      tester,
      const LessonCompletionScreen(),
      api: api,
      initial: afterCall,
    );

    await tapText(tester, '完了を記録する');

    expect(find.text('このレッスンの完了はすでに記録されています。'), findsOneWidget);
    expect(find.text('完了を記録する'), findsOneWidget);
  });
}
