import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/lesson_review_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final afterCompletion = StudentFlowState(
    step: FlowStep.lessonReview,
    student: Fixtures.student,
    bookedSlot: Fixtures.slot,
    booking: Fixtures.bookingConfirmed,
    session: Fixtures.sessionEnded,
    lesson: Fixtures.liveLesson,
  );

  Finder sendButton() => find.widgetWithText(FilledButton, '評価を送る');

  testWidgets('点と一言がそろうまで送れず、送ると submitLessonReview を呼んで入力できなくする', (
    tester,
  ) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: api,
      initial: afterCompletion,
    );

    expect(tester.widget<FilledButton>(sendButton()).onPressed, isNull);
    await tapText(tester, '4');
    expect(tester.widget<FilledButton>(sendButton()).onPressed, isNull);
    await tester.enterText(find.byType(TextField), '   ');
    await tester.pump();
    expect(tester.widget<FilledButton>(sendButton()).onPressed, isNull);

    await tester.enterText(find.byType(TextField), '  丁寧でした  ');
    await tester.pump();
    await tapText(tester, '評価を送る');

    expect(api.calls, ['submitLessonReview']);
    expect((api.requests.single as SubmitLessonReviewRequest).toJson(), {
      'lesson_id': Fixtures.liveLesson.lessonId,
      'rating': 4,
      'comment': '丁寧でした',
    });
    expect(find.text('評価を送りました'), findsOneWidget);
    expect(find.text('評価: 4 / 5'), findsOneWidget);
    expect(find.text('一言: 丁寧でした'), findsOneWidget);
    expect(sendButton(), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(container.read(studentFlowProvider).reviewedLessonIds, {
      Fixtures.liveLesson.lessonId,
    });

    await tapText(tester, '月額契約へ進む');
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  testWidgets('409 lesson_already_reviewed なら文言を出し、評価済みとして入力できなくする', (
    tester,
  ) async {
    final api = FakeStudentApi(
      failures: {'submitLessonReview': Fixtures.alreadyReviewed},
    );
    final container = await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: api,
      initial: afterCompletion,
    );

    await tapText(tester, '5');
    await tester.enterText(find.byType(TextField), 'よかった');
    await tester.pump();
    await tapText(tester, '評価を送る');

    expect(find.text('このレッスンはすでに評価しています。'), findsOneWidget);
    expect(find.text('このレッスンは評価済みのため、入力できません。'), findsOneWidget);
    expect(sendButton(), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(container.read(studentFlowProvider).lessonReviewed, isTrue);
  });

  testWidgets('ほかの失敗では評価済みにせず、入力を残して送り直せる', (tester) async {
    final api = FakeStudentApi(
      failures: {'submitLessonReview': Fixtures.validationFailed},
    );
    final container = await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: api,
      initial: afterCompletion,
    );

    await tapText(tester, '3');
    await tester.enterText(find.byType(TextField), 'ふつう');
    await tester.pump();
    await tapText(tester, '評価を送る');

    expect(find.text('入力内容を確かめてください。'), findsOneWidget);
    expect(container.read(studentFlowProvider).lessonReviewed, isFalse);
    expect(tester.widget<FilledButton>(sendButton()).onPressed, isNotNull);
  });

  testWidgets('この端末で評価済みのレッスンは開いた時点で入力させない', (tester) async {
    await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: FakeStudentApi(),
      initial: afterCompletion.copyWith(
        reviewedLessonIds: {Fixtures.liveLesson.lessonId},
      ),
    );

    expect(find.text('このレッスンは評価済みのため、入力できません。'), findsOneWidget);
    expect(sendButton(), findsNothing);
  });
}
