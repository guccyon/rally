import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/slot_search_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final registered = StudentFlowState(
    step: FlowStep.slotSearch,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  testWidgets('スポーツで絞って検索し、スロットを体験予約すると確定待ちに進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    expect(find.text('スポーツを選ぶと、予約できるスロットを表示します。'), findsOneWidget);
    await tapText(tester, 'テニス');
    expect(api.requests.last, Sport.tennis);
    expect(find.text('体験予約する'), findsOneWidget);

    await tapText(tester, '体験予約する');

    final request = api.requests.last as CreateBookingRequest;
    expect(request.toJson(), {'slot_id': Fixtures.slotId, 'is_trial': true});
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.bookingPending);
    expect(state.booking!.status, BookingStatus.pending);
  });

  testWidgets('そのスポーツのスロットがなければその旨を出す', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'サッカー');

    expect(api.requests.last, Sport.soccer);
    expect(find.text('予約できるスロットはありません。'), findsOneWidget);
  });

  testWidgets('先に予約されたスロットなら文言を出す', (tester) async {
    final api = FakeStudentApi(
      failures: {
        'createBooking': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.slotAlreadyBooked,
            message: 'slot already booked',
          ),
        ),
      },
    );
    final container = await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');
    await tapText(tester, '体験予約する');

    expect(find.text('このスロットはすでに予約されました。別のスロットを選んでください。'), findsOneWidget);
    expect(container.read(studentFlowProvider).step, FlowStep.slotSearch);
  });
}
