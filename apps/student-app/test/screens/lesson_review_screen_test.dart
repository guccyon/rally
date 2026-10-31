import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/lesson_review_screen.dart';
import 'package:rally_student_app/state/lesson_review.dart';
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
    await tapText(tester, '4　よかった');
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
    expect(find.text('評価: 4（よかった）'), findsOneWidget);
    expect(find.text('一言: 丁寧でした'), findsOneWidget);
    expect(sendButton(), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(container.read(lessonReviewProvider).reviewedLessonIds, {
      Fixtures.liveLesson.lessonId,
    });
    expect(
      () => container.read(lessonReviewProvider).reviewedLessonIds.add('other'),
      throwsUnsupportedError,
    );

    await tapText(tester, '月額契約へ進む');
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  testWidgets('点と言葉の目安を 1 行にした選択肢を 5 → 1 の順に縦に並べ、選んだものに印を付ける', (tester) async {
    await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: FakeStudentApi(),
      initial: afterCompletion,
    );

    const choices = ['5　とてもよかった', '4　よかった', '3　ふつう', '2　少し物足りなかった', '1　不満だった'];
    final tiles = find.byType(RadioListTile<int>);
    expect(tiles, findsNWidgets(5));
    expect([
      for (final tile in tester.widgetList<RadioListTile<int>>(tiles))
        (tile.title! as Text).data,
    ], choices);
    final tops = [
      for (final choice in choices) tester.getTopLeft(find.text(choice)).dy,
    ];
    for (var i = 1; i < tops.length; i++) {
      expect(tops[i], greaterThan(tops[i - 1]));
    }
    expect(
      find.text(
        'コーチの指導を 1（不満だった）〜5（とてもよかった） で評価し、一言を添えてください。評価は 1 回だけで、送ったあとは変えられません。',
      ),
      findsOneWidget,
    );
    expect(find.textContaining('選んだ評価'), findsNothing);

    int? selected() => RadioGroup.maybeOf<int>(
      tester.element(find.byType(RadioListTile<int>).first),
    )?.groupValue;
    expect(selected(), isNull);

    await tapText(tester, '2　少し物足りなかった');
    expect(selected(), 2);

    await tapText(tester, '5　とてもよかった');
    expect(selected(), 5);
  });

  testWidgets('幅の狭い端末でも選択肢の言葉が折り返さず、はみ出さない', (tester) async {
    tester.view.physicalSize = const Size(320, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);

    await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: FakeStudentApi(),
      initial: afterCompletion,
    );
    await tapText(tester, '2　少し物足りなかった');

    expect(tester.takeException(), isNull);
    final line = tester.getSize(find.text('3　ふつう')).height;
    expect(tester.getSize(find.text('2　少し物足りなかった')).height, line);
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

    await tapText(tester, '5　とてもよかった');
    await tester.enterText(find.byType(TextField), 'よかった');
    await tester.pump();
    await tapText(tester, '評価を送る');

    expect(find.text('このレッスンはすでに評価しています。'), findsOneWidget);
    expect(find.text('このレッスンは評価済みのため、入力できません。'), findsOneWidget);
    expect(sendButton(), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(
      container
          .read(lessonReviewProvider)
          .isReviewed(Fixtures.liveLesson.lessonId),
      isTrue,
    );
    expect(
      () => container.read(lessonReviewProvider).reviewedLessonIds.add('other'),
      throwsUnsupportedError,
    );
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

    await tapText(tester, '3　ふつう');
    await tester.enterText(find.byType(TextField), 'ふつう');
    await tester.pump();
    await tapText(tester, '評価を送る');

    expect(find.text('入力内容を確かめてください。'), findsOneWidget);
    expect(
      container
          .read(lessonReviewProvider)
          .isReviewed(Fixtures.liveLesson.lessonId),
      isFalse,
    );
    expect(tester.widget<FilledButton>(sendButton()).onPressed, isNotNull);
  });

  testWidgets('この端末で評価済みのレッスンは開いた時点で入力させない', (tester) async {
    await pumpScreen(
      tester,
      const LessonReviewScreen(),
      api: FakeStudentApi(),
      initial: afterCompletion,
      review: LessonReviewState(
        reviewedLessonIds: {Fixtures.liveLesson.lessonId},
      ),
    );

    expect(find.text('このレッスンは評価済みのため、入力できません。'), findsOneWidget);
    expect(sendButton(), findsNothing);
  });
}
