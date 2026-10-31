import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/subscription_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  Future<FakeStudentApi> pumpSubscription(
    WidgetTester tester, {
    StudentFlowState? initial,
    FakeStudentApi? api,
  }) async {
    api ??= FakeStudentApi();
    await pumpScreen(
      tester,
      const SubscriptionScreen(),
      api: api,
      initial:
          initial ??
          StudentFlowState(
            step: FlowStep.subscription,
            student: Fixtures.student,
          ),
    );
    // 開いた直後に読むプランの一覧を待つ
    await tester.pumpAndSettle();
    return api;
  }

  RadioListTile<SubscriptionPlan> planTile(WidgetTester tester, String text) =>
      tester.widget(
        find.ancestor(
          of: find.text(text),
          matching: find.byType(RadioListTile<SubscriptionPlan>),
        ),
      );

  testWidgets('プランは月のビデオ通話の回数を添えて出し、既定のスタンダードで始めると今月の通話の枠を出す', (tester) async {
    final api = await pumpSubscription(tester);

    expect(find.text('スポーツ: テニス'), findsOneWidget);
    expect(find.text('スタンダード　ビデオ通話 月 2 回'), findsOneWidget);
    expect(find.text('プラス　ビデオ通話 月 4 回'), findsOneWidget);
    final group = tester.widget<RadioGroup<SubscriptionPlan>>(
      find.byType(RadioGroup<SubscriptionPlan>),
    );
    expect(group.groupValue, SubscriptionPlan.standard);

    await tapText(tester, '月額契約を始める');

    expect(api.calls, ['listSubscriptionPlans', 'startSubscription']);
    final request = api.requests.single as StartSubscriptionRequest;
    expect(request.toJson(), {
      'sport': 'tennis',
      'plan': 'standard',
      'payment_method_token': mockPaymentMethodToken,
    });
    expect(find.text('月額契約を開始しました'), findsOneWidget);
    expect(find.text('プラン: スタンダード'), findsOneWidget);
    expect(find.text('今月のビデオ通話: 月 2 回のうち残り 2 回'), findsOneWidget);
    expect(find.text('月額契約を始める'), findsNothing);
    expect(find.byType(RadioGroup<SubscriptionPlan>), findsNothing);
  });

  testWidgets('プラスを選んで始めると plus で startSubscription を呼び、月 4 回の枠を出す', (
    tester,
  ) async {
    final api = await pumpSubscription(tester);

    await tapText(tester, 'プラス　ビデオ通話 月 4 回');
    expect(planTile(tester, 'プラス　ビデオ通話 月 4 回').value, SubscriptionPlan.plus);
    await tapText(tester, '月額契約を始める');

    final request = api.requests.single as StartSubscriptionRequest;
    expect(request.plan, SubscriptionPlan.plus);
    expect(request.toJson()['plan'], 'plus');
    expect(find.text('プラン: プラス'), findsOneWidget);
    expect(find.text('今月のビデオ通話: 月 4 回のうち残り 4 回'), findsOneWidget);
  });

  testWidgets('プランに添える回数は、アプリに持たず listSubscriptionPlans の応答の値を出す', (
    tester,
  ) async {
    await pumpSubscription(
      tester,
      api: FakeStudentApi(
        subscriptionPlans: const [
          SubscriptionPlanDetail(
            plan: SubscriptionPlan.standard,
            liveLessonsPerMonth: 3,
          ),
          SubscriptionPlanDetail(
            plan: SubscriptionPlan.plus,
            liveLessonsPerMonth: 6,
          ),
        ],
      ),
    );

    expect(find.text('スタンダード　ビデオ通話 月 3 回'), findsOneWidget);
    expect(find.text('プラス　ビデオ通話 月 6 回'), findsOneWidget);
    expect(find.text('スタンダード　ビデオ通話 月 2 回'), findsNothing);
  });

  testWidgets('プランの一覧を読めなければ、回数を出さずに名前だけで選べて、その旨をプランの欄に出す', (tester) async {
    final api = await pumpSubscription(
      tester,
      api: FakeStudentApi(
        failures: {
          'listSubscriptionPlans': const StudentApiException(statusCode: 500),
        },
      ),
    );

    expect(find.text('スタンダード'), findsOneWidget);
    expect(find.text('プラス'), findsOneWidget);
    expect(find.textContaining('ビデオ通話 月'), findsNothing);
    expect(
      find.text('プランごとのビデオ通話の回数を読み込めませんでした。プランの名前で選んで始められます。'),
      findsOneWidget,
    );

    await tapText(tester, 'プラス');
    expect(planTile(tester, 'プラス').value, SubscriptionPlan.plus);
    await tapText(tester, '月額契約を始める');

    final request = api.requests.single as StartSubscriptionRequest;
    expect(request.plan, SubscriptionPlan.plus);
    expect(find.text('月額契約を開始しました'), findsOneWidget);
    expect(find.text('今月のビデオ通話: 月 4 回のうち残り 4 回'), findsOneWidget);
  });

  testWidgets('今月の残り回数は、受講権のプランの上限と合わせて出す', (tester) async {
    final started = Fixtures.subscriptionStartedWith(SubscriptionPlan.plus);
    final entitlement = started.entitlement;
    await pumpSubscription(
      tester,
      initial: StudentFlowState(
        step: FlowStep.subscription,
        student: Fixtures.student,
        subscription: started.subscription,
        entitlement: Entitlement(
          entitlementId: entitlement.entitlementId,
          studentId: entitlement.studentId,
          contractType: entitlement.contractType,
          sport: entitlement.sport,
          plan: entitlement.plan,
          unlimitedVideoAndChat: true,
          liveLessonQuota: const LiveLessonQuota(
            month: '2026-10',
            limit: 4,
            used: 1,
            remaining: 3,
          ),
        ),
      ),
    );

    expect(find.text('今月のビデオ通話: 月 4 回のうち残り 3 回'), findsOneWidget);
    expect(find.text('月額契約を始める'), findsNothing);
  });

  testWidgets('契約の後に開いたときは、プランの一覧を読まない', (tester) async {
    final started = Fixtures.subscriptionStarted;
    final api = await pumpSubscription(
      tester,
      initial: StudentFlowState(
        step: FlowStep.subscription,
        student: Fixtures.student,
        subscription: started.subscription,
        entitlement: started.entitlement,
      ),
    );

    expect(api.calls, isEmpty);
    expect(find.text('プラン: スタンダード'), findsOneWidget);
  });
}
