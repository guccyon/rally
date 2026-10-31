import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final atHome = StudentFlowState(
    step: FlowStep.home,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  Future<void> openProfile(WidgetTester tester) async {
    await tester.tap(find.byTooltip('プロフィール'));
    await tester.pumpAndSettle();
  }

  String fieldText(WidgetTester tester) =>
      tester.widget<TextField>(find.byType(TextField)).controller!.text;

  FilledButton saveButton(WidgetTester tester) =>
      tester.widget<FilledButton>(find.byType(FilledButton));

  testWidgets('アプリバーから開くと getMyStudentProfile の表示名とスポーツを出し、閉じると開いた画面に戻る', (
    tester,
  ) async {
    final api = FakeStudentApi(
      student: Student(
        studentId: Fixtures.studentId,
        displayName: 'やまだ',
        sport: Sport.tennis,
        createdAt: Fixtures.now,
      ),
    );
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
    );

    await openProfile(tester);

    expect(api.calls, ['getMyStudentProfile']);
    expect(find.text('プロフィール'), findsOneWidget);
    // サーバーから読み直した表示名を出す
    expect(fieldText(tester), 'やまだ');
    expect(find.text('スポーツ: テニス'), findsOneWidget);
    // 変えていなければ保存できない
    expect(saveButton(tester).onPressed, isNull);
    // プロフィールの画面からはほかの脇の画面を重ねて開かない
    expect(find.byTooltip('プロフィール'), findsNothing);
    expect(find.byTooltip('通知の設定'), findsNothing);

    await tester.tap(find.byTooltip('プロフィールを閉じる'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.home);
    expect(find.text('やまださん、こんにちは'), findsOneWidget);
  });

  testWidgets(
    '表示名を変えて保存すると updateMyStudentProfile で書いたとおりに送り、保存中・成功を出し、ホームの表示も変わる',
    (tester) async {
      final saving = Completer<void>();
      final api = FakeStudentApi()..profileSaveGate = saving.future;
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: atHome,
      );
      await openProfile(tester);
      expect(fieldText(tester), '山田');

      await tester.enterText(find.byType(TextField), '山田 太郎');
      await tester.pump();
      await tester.tap(find.text('保存する'));
      await tester.pump();

      expect(find.text('保存しています…'), findsOneWidget);
      expect(saveButton(tester).onPressed, isNull);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);

      saving.complete();
      await tester.pumpAndSettle();

      expect(api.calls, ['getMyStudentProfile', 'updateMyStudentProfile']);
      expect((api.requests.single as UpdateStudentProfileRequest).toJson(), {
        'display_name': '山田 太郎',
      });
      expect(find.text('表示名を保存しました。'), findsOneWidget);
      expect(find.text('保存する'), findsOneWidget);
      expect(container.read(studentFlowProvider).student!.displayName, '山田 太郎');

      // 入力を変えたら成功の文言は消える
      await tester.enterText(find.byType(TextField), '山田 次郎');
      await tester.pump();
      expect(find.text('表示名を保存しました。'), findsNothing);

      await tester.tap(find.byTooltip('プロフィールを閉じる'));
      await tester.pumpAndSettle();
      expect(find.text('山田 太郎さん、こんにちは'), findsOneWidget);
    },
  );

  testWidgets('422 で断られたらサーバーの文言を出し、表示名は変えず、直して保存し直せる', (tester) async {
    final failures = <String, StudentApiException>{
      'updateMyStudentProfile': const StudentApiException(
        statusCode: 422,
        error: ApiError(
          code: ErrorCode.validationFailed,
          message: '表示名は 50 文字まで',
        ),
      ),
    };
    final api = FakeStudentApi(failures: failures);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
    );
    await openProfile(tester);

    final tooLong = 'あ' * 51;
    await tester.enterText(find.byType(TextField), tooLong);
    await tester.pump();
    await tapText(tester, '保存する');

    expect(api.calls.last, 'updateMyStudentProfile');
    expect(find.text('表示名は 50 文字まで'), findsOneWidget);
    expect(find.text('表示名を保存しました。'), findsNothing);
    expect(container.read(studentFlowProvider).student!.displayName, '山田');
    // 入力はそのまま残し、直して保存し直せる
    expect(fieldText(tester), tooLong);
    expect(saveButton(tester).onPressed, isNotNull);

    failures.clear();
    await tester.enterText(find.byType(TextField), 'あ' * 50);
    await tester.pump();
    await tapText(tester, '保存する');

    expect(find.text('表示名は 50 文字まで'), findsNothing);
    expect(find.text('表示名を保存しました。'), findsOneWidget);
    expect(container.read(studentFlowProvider).student!.displayName, 'あ' * 50);
  });

  testWidgets('読み込みに失敗したら知らせ、登録で受け取った表示名のまま変えられる', (tester) async {
    final api = FakeStudentApi(
      failures: {'getMyStudentProfile': Fixtures.unreachable},
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
    );
    await openProfile(tester);

    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    expect(fieldText(tester), '山田');

    await tester.enterText(find.byType(TextField), 'やまだ');
    await tester.pump();
    await tapText(tester, '保存する');

    expect(api.calls, ['getMyStudentProfile', 'updateMyStudentProfile']);
    expect(find.text('表示名を保存しました。'), findsOneWidget);
  });
}
