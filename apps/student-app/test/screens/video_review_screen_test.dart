import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/video_review_screen.dart';
import 'package:rally_student_app/state/next_actions.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  testWidgets('未返却なら待ちを出し、確かめ直して返却されたらコメントを出して完了記録に進める', (tester) async {
    final api = FakeStudentApi(
      reviews: [Fixtures.reviewReturned],
      nextActions: [Fixtures.nextRecordAsyncCompletion],
    );
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
    // メニューを選ばずに送った動画には、メニューを出さない
    expect(find.textContaining('メニュー: '), findsNothing);

    await tapText(tester, '添削結果を確かめる');

    expect(find.text('AI の一次フィードバック'), findsNothing);
    // 返却されたので、完了の記録へ進めるかをサーバーの次の操作で確かめる
    expect(api.calls, ['getVideoReview', 'getMyNextActions']);
    expect(api.requests.single, Fixtures.videoId);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);

    await tapText(tester, '完了の記録へ進む');
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.lessonCompletion);
    expect(
      (state.completing! as AsyncLessonCompletion).videoId,
      Fixtures.videoId,
    );
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
      nextActions: NextActionsState(
        actions: [Fixtures.nextRecordAsyncCompletion],
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

  testWidgets('動画を送ったときにメニューを選んでいれば「メニュー: 名前」を出す', (tester) async {
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
        review: Fixtures.reviewReturnedWithMenu,
      ),
    );

    expect(find.text('メニュー: サーブの動画添削'), findsOneWidget);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
  });

  testWidgets('返却済みでも、サーバーの次の操作にその動画の完了の記録がなければ「完了の記録へ進む」を出さない', (
    tester,
  ) async {
    for (final nextActions in [
      // 記録済み（次の操作にない）
      NextActionsState(actions: const []),
      // 別の動画の完了の記録だけがある
      NextActionsState(
        actions: [
          const NextAction(
            kind: NextActionKind.recordLessonCompletion,
            lessonType: LessonType.asyncLesson,
            videoId: 'another-video',
          ),
        ],
      ),
      // 次の操作を読めない（二重の記録を避けて出さない）
      NextActionsState(failed: true),
    ]) {
      await pumpScreen(
        tester,
        const VideoReviewScreen(),
        api: FakeStudentApi(),
        initial: StudentFlowState(
          step: FlowStep.videoReview,
          student: Fixtures.student,
          video: Fixtures.video,
          review: Fixtures.reviewReturned,
        ),
        nextActions: nextActions,
      );

      expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
      expect(find.text('完了の記録へ進む'), findsNothing);
    }
  });
}
