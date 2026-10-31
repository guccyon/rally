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

  testWidgets('登録すると registerAccount を呼び、ホームの欄を読んでホームに進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const RegisterScreen(),
      api: api,
    );

    await fillForm(tester);
    await tapText(tester, '登録する');

    expect(api.calls, [
      'registerAccount',
      'listMyBookings',
      'getMyEntitlement',
      'listMessageThreads',
      'listMyVideos',
    ]);
    final request = api.requests.first as RegisterAccountRequest;
    expect(request.toJson(), {
      'email': 'yamada@example.com',
      'display_name': '山田',
      'sport': 'tennis',
    });
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.home);
    expect(state.accessToken, Fixtures.registered.accessToken);
    expect(state.searchSport, isNull);
  });

  testWidgets('水泳を選んで登録すると、水泳で登録し、ホームからスロットを探すと水泳で絞る', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const RegisterScreen(),
      api: api,
    );

    await tester.enterText(
      find.widgetWithText(TextFormField, 'メールアドレス'),
      'yamada@example.com',
    );
    await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
    await tapText(tester, '水泳');
    await tapText(tester, '登録する');

    final request = api.requests.first as RegisterAccountRequest;
    expect(request.toJson()['sport'], 'swimming');
    await container.read(studentFlowProvider.notifier).goToSlotSearch();
    expect(api.calls.last, 'searchSlots');
    expect(api.requests.last, Sport.swimming);
    expect(api.requests.whereType<Sport>().last, Sport.swimming);
    expect(container.read(studentFlowProvider).searchSport, Sport.swimming);
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
