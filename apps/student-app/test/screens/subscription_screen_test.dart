import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/subscription_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  Future<FakeStudentApi> pumpSubscription(
    WidgetTester tester, {
    StudentFlowState? initial,
  }) async {
    final api = FakeStudentApi();
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
}
