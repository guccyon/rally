import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/register_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  Future<void> fillForm(WidgetTester tester) async {
    await tester.enterText(
      find.widgetWithText(TextFormField, 'メールアドレス'),
      'yamada@example.com',
    );
    await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
    await tapText(tester, 'テニス');
  }

  testWidgets('登録すると registerAccount を呼び、スロット検索に進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const RegisterScreen(),
      api: api,
    );

    await fillForm(tester);
    await tapText(tester, '登録する');

    expect(api.calls, ['registerAccount', 'searchSlots']);
    final request = api.requests.first as RegisterAccountRequest;
    expect(request.toJson(), {
      'email': 'yamada@example.com',
      'display_name': '山田',
      'sport': 'tennis',
    });
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.slotSearch);
    expect(state.accessToken, Fixtures.registered.accessToken);
    expect(state.searchSport, Sport.tennis);
  });

  testWidgets('未入力なら API を呼ばずに入力を促す', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(tester, const RegisterScreen(), api: api);

    await tapText(tester, '登録する');

    expect(api.calls, isEmpty);
    expect(find.text('メールアドレスを入力してください'), findsOneWidget);
    expect(find.text('表示名を入力してください'), findsOneWidget);
    expect(find.text('スポーツを選んでください'), findsOneWidget);
  });

  testWidgets('登録済みのメールアドレスなら文言を出してこの画面に留まる', (tester) async {
    final api = FakeStudentApi(
      failures: {
        'registerAccount': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.emailAlreadyRegistered,
            message: 'email already registered',
          ),
        ),
      },
    );
    final container = await pumpScreen(
      tester,
      const RegisterScreen(),
      api: api,
    );

    await fillForm(tester);
    await tapText(tester, '登録する');

    expect(find.text('このメールアドレスはすでに登録されています。'), findsOneWidget);
    expect(container.read(studentFlowProvider).step, FlowStep.register);
  });
}
