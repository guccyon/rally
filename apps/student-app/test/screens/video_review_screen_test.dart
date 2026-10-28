import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/screens/video_review_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  testWidgets('未返却なら待ちを出し、確かめ直して返却されたらコメントを出して完了記録に進める', (tester) async {
    final api = FakeStudentApi(reviews: [Fixtures.reviewReturned]);
    final container = await pumpScreen(
      tester,
      const VideoReviewScreen(),
      api: api,
      initial: StudentFlowState(
        step: FlowStep.videoReview,
        student: Fixtures.student,
        bookedSlot: Fixtures.slot,
        booking: Fixtures.bookingConfirmed,
        video: Fixtures.video,
        review: Fixtures.reviewInProgress,
      ),
    );

    expect(find.text('コーチが添削しています。返却されるまでお待ちください。'), findsOneWidget);
    expect(find.text('完了の記録へ進む'), findsNothing);

    await tapText(tester, '添削結果を確かめる');

    expect(find.text('AI の一次フィードバック'), findsNothing);
    expect(api.calls, ['getVideoReview']);
    expect(api.requests.single, Fixtures.videoId);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);

    await tapText(tester, '完了の記録へ進む');
    expect(container.read(studentFlowProvider).step, FlowStep.lessonCompletion);
  });

  testWidgets('未返却でも AI の一次フィードバックがあれば、待ちと一緒に別の枠で出す', (tester) async {
    await pumpScreen(
      tester,
      const VideoReviewScreen(),
      api: FakeStudentApi(),
      initial: StudentFlowState(
        step: FlowStep.videoReview,
        student: Fixtures.student,
        bookedSlot: Fixtures.slot,
        booking: Fixtures.bookingConfirmed,
        video: Fixtures.video,
        review: Fixtures.reviewInProgressWithAi,
      ),
    );

    expect(find.text('コーチが添削しています。返却されるまでお待ちください。'), findsOneWidget);
    expect(find.text('添削結果を確かめる'), findsOneWidget);
    expect(find.text('AI の一次フィードバック'), findsOneWidget);
    expect(find.text('AI が自動で作った文です。コーチの添削ではありません。'), findsOneWidget);
    expect(find.text('フォームの全体の流れは安定しています'), findsOneWidget);
    expect(find.text('コーチの添削'), findsNothing);
    expect(find.text('完了の記録へ進む'), findsNothing);
  });

  testWidgets('返却後は コーチの添削と AI の一次フィードバックを別の枠で両方出す', (tester) async {
    await pumpScreen(
      tester,
      const VideoReviewScreen(),
      api: FakeStudentApi(),
      initial: StudentFlowState(
        step: FlowStep.videoReview,
        student: Fixtures.student,
        bookedSlot: Fixtures.slot,
        booking: Fixtures.bookingConfirmed,
        video: Fixtures.video,
        review: Fixtures.reviewReturnedWithAi,
      ),
    );

    expect(find.text('コーチの添削'), findsOneWidget);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
    expect(find.text('AI の一次フィードバック'), findsOneWidget);
    expect(find.text('フォームの全体の流れは安定しています'), findsOneWidget);
    expect(find.text('コーチが添削しています。返却されるまでお待ちください。'), findsNothing);
    expect(find.text('完了の記録へ進む'), findsOneWidget);

    // コーチの添削と AI の文は別の Card に入る
    final coachCard = find.ancestor(
      of: find.text('テイクバックをもう少し早く'),
      matching: find.byType(Card),
    );
    final aiCard = find.ancestor(
      of: find.text('フォームの全体の流れは安定しています'),
      matching: find.byType(Card),
    );
    expect(coachCard, findsOneWidget);
    expect(aiCard, findsOneWidget);
    expect(tester.widget(coachCard), isNot(same(tester.widget(aiCard))));
  });
}
