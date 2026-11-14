import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';
import 'package:rally_student_app/ui/labels.dart';

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

    expect(api.calls, ['getMyStudentProfile', 'getMyEntitlement']);
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

      expect(api.calls, [
        'getMyStudentProfile',
        'getMyEntitlement',
        'updateMyStudentProfile',
      ]);
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

    expect(api.calls, [
      'getMyStudentProfile',
      'getMyEntitlement',
      'updateMyStudentProfile',
    ]);
    expect(find.text('表示名を保存しました。'), findsOneWidget);
  });

  testWidgets('月額契約がなければ月額契約の欄に「月額契約へ進む」の行を出し、押すと月額契約の画面を開く', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
    );
    await openProfile(tester);

    expect(find.text('月額契約'), findsOneWidget);
    expect(find.text('まだ月額契約はありません'), findsOneWidget);
    // 表示名の欄はそのまま使える
    expect(fieldText(tester), '山田');

    await tapText(tester, '月額契約へ進む');

    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
    expect(find.text('月額契約を始める'), findsOneWidget);
  });

  testWidgets('この端末で月額契約を始めたあとは、月額契約の欄にプランと開始の日を出し、「月額契約へ進む」を出さない', (
    tester,
  ) async {
    final started = Fixtures.subscriptionStartedWith(SubscriptionPlan.plus);
    final api = FakeStudentApi(entitlement: started.entitlement);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
      subscription: SubscriptionState(
        subscription: started.subscription,
        entitlement: started.entitlement,
      ),
    );
    await openProfile(tester);

    expect(find.text('月額契約へ進む'), findsNothing);
    expect(
      find.text(
        subscriptionSummaryLabel(
          SubscriptionPlan.plus,
          startedAt: started.subscription.startedAt,
        ),
      ),
      findsOneWidget,
    );
    expect(find.textContaining('プラン: プラス・'), findsOneWidget);

    // 押すと契約の中身を見る画面
    await tester.tap(find.textContaining('プラン: プラス・'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
    expect(find.text('月額契約を開始しました'), findsOneWidget);
  });

  testWidgets('サーバーの受講権だけがある（開き直した）ときはプランを出し、行は押せない（契約の前の画面に戻さない）', (
    tester,
  ) async {
    final api = FakeStudentApi(
      entitlement: Fixtures.subscriptionStarted.entitlement,
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
    );
    await openProfile(tester);

    expect(find.text('月額契約へ進む'), findsNothing);
    expect(find.text('プラン: スタンダード'), findsOneWidget);
    final row = tester.widget<ListTile>(
      find.ancestor(
        of: find.text('プラン: スタンダード'),
        matching: find.byType(ListTile),
      ),
    );
    expect(row.onTap, isNull);
  });

  testWidgets('受講権を読めなければ月額契約の欄にだけその旨を出し、表示名は使えるまま', (tester) async {
    final api = FakeStudentApi(
      failures: {'getMyEntitlement': Fixtures.unreachable},
    );
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: atHome,
    );
    await openProfile(tester);

    expect(
      find.text('月額契約を読み込めませんでした。プロフィールを開き直すと、もう一度読み込めます。'),
      findsOneWidget,
    );
    expect(find.text('月額契約へ進む'), findsNothing);
    // 失敗の文言は出さない（欄にだけ旨を出す）
    expect(container.read(studentFlowProvider).errorMessage, isNull);

    await tester.enterText(find.byType(TextField), 'やまだ');
    await tester.pump();
    await tapText(tester, '保存する');
    expect(find.text('表示名を保存しました。'), findsOneWidget);
  });

  testWidgets('受講権をまだ読めていないあいだは、月額契約の欄に「月額契約を読み込んでいます…」を出す', (tester) async {
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: FakeStudentApi(),
      initial: atHome.copyWith(step: FlowStep.profile),
    );

    expect(find.text('月額契約を読み込んでいます…'), findsOneWidget);
    expect(find.text('月額契約へ進む'), findsNothing);
  });
}
