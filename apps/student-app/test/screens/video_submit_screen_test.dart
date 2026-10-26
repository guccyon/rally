import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/video_submit_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  // 登録はテニスだが、サッカーのスロットを体験予約した生徒
  final soccerSlot = Slot(
    slotId: Fixtures.slotId,
    coachId: Fixtures.coachId,
    sport: Sport.soccer,
    startsAt: Fixtures.slot.startsAt,
    endsAt: Fixtures.slot.endsAt,
  );

  testWidgets('動画を選んで（モック）送付すると、体験予約のコーチとスロットのスポーツで送って添削結果に進む', (tester) async {
    final api = FakeStudentApi(reviews: [Fixtures.reviewInProgress]);
    final container = await pumpScreen(
      tester,
      const VideoSubmitScreen(),
      api: api,
      initial: StudentFlowState(
        step: FlowStep.videoSubmit,
        student: Fixtures.student,
        bookedSlot: soccerSlot,
        booking: Fixtures.bookingConfirmed,
      ),
    );

    final submit = find.widgetWithText(FilledButton, '送付する');
    expect(tester.widget<FilledButton>(submit).onPressed, isNull);

    await tapText(tester, '動画を選ぶ（モック）');
    expect(find.text('選んだ動画: swing.mp4'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'フォアハンドを見てください');
    await tapText(tester, '送付する');

    expect(api.calls, ['submitVideo', 'getVideoReview']);
    final request = api.requests.first as SubmitVideoRequest;
    expect(request.toJson(), {
      'coach_id': Fixtures.coachId,
      'sport': 'soccer',
      'content_type': 'video/mp4',
      'note': 'フォアハンドを見てください',
    });
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.videoReview);
    expect(state.review!.status, VideoReviewStatus.inProgress);
  });
}
