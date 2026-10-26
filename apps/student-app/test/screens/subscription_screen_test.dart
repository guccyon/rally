import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/subscription_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  testWidgets('月額契約を始めると、モックの決済トークンで startSubscription を呼び、今月の通話の枠を出す', (
    tester,
  ) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const SubscriptionScreen(),
      api: api,
      initial: StudentFlowState(
        step: FlowStep.subscription,
        student: Fixtures.student,
      ),
    );

    expect(find.text('スポーツ: テニス'), findsOneWidget);
    await tapText(tester, '月額契約を始める');

    final request = api.requests.single as StartSubscriptionRequest;
    expect(request.toJson(), {
      'sport': 'tennis',
      'payment_method_token': mockPaymentMethodToken,
    });
    expect(find.text('月額契約を開始しました'), findsOneWidget);
    expect(find.text('今月のビデオ通話: 残り 2 / 2 回'), findsOneWidget);
    expect(find.text('月額契約を始める'), findsNothing);
  });
}
