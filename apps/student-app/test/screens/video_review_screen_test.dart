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

    expect(api.calls, ['getVideoReview']);
    expect(api.requests.single, Fixtures.videoId);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);

    await tapText(tester, '完了の記録へ進む');
    expect(container.read(studentFlowProvider).step, FlowStep.lessonCompletion);
  });
}
