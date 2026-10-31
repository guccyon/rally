import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final afterSubscription = StudentFlowState(
    step: FlowStep.subscription,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );
  final subscribed = SubscriptionState(
    subscription: Fixtures.subscriptionStarted.subscription,
    entitlement: Fixtures.subscriptionStarted.entitlement,
  );

  const labels = [
    '予約が確定したとき',
    '動画の添削が返ってきたとき',
    '担当コーチからメッセージが届いたとき',
    '本部からのお知らせが届いたとき',
  ];

  Future<void> openSettings(WidgetTester tester) async {
    await tester.tap(find.byTooltip('通知の設定'));
    await tester.pumpAndSettle();
  }

  bool switchOf(WidgetTester tester, String label) => tester
      .widget<SwitchListTile>(find.widgetWithText(SwitchListTile, label))
      .value;

  testWidgets(
    'アプリバーから開くと getMyNotificationSettings で読み込み、種類ごとのスイッチを出し、閉じると開いた画面に戻る',
    (tester) async {
      final api = FakeStudentApi(
        notificationSettings: Fixtures.notificationSettings.withKind(
          NotificationKind.messageReceived,
          false,
        ),
      );
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: afterSubscription,
        subscription: subscribed,
      );

      await openSettings(tester);

      expect(api.calls, ['getMyNotificationSettings']);
      expect(find.text('通知の設定'), findsOneWidget);
      expect([
        for (final tile in tester.widgetList<SwitchListTile>(
          find.byType(SwitchListTile),
        ))
          (tile.title! as Text).data,
      ], labels);
      expect(switchOf(tester, '予約が確定したとき'), isTrue);
      expect(switchOf(tester, '動画の添削が返ってきたとき'), isTrue);
      expect(switchOf(tester, '担当コーチからメッセージが届いたとき'), isFalse);
      // 設定の画面からはチャットや設定を重ねて開かない
      expect(find.byTooltip('チャット'), findsNothing);
      expect(find.byTooltip('通知の設定'), findsNothing);

      await tester.tap(find.byTooltip('通知の設定を閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.subscription);
    },
  );

  testWidgets('切り替えるとその種類だけを updateMyNotificationSettings ですぐ保存する', (
    tester,
  ) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openSettings(tester);

    await tapText(tester, '動画の添削が返ってきたとき');

    expect(api.calls, [
      'getMyNotificationSettings',
      'updateMyNotificationSettings',
    ]);
    expect(
      (api.requests.single as UpdateNotificationSettingsRequest).toJson(),
      {'video_review_returned': false},
    );
    expect(switchOf(tester, '動画の添削が返ってきたとき'), isFalse);
    expect(switchOf(tester, '予約が確定したとき'), isTrue);
    expect(
      container
          .read(studentFlowProvider)
          .notificationSettings!
          .videoReviewReturned,
      isFalse,
    );

    await tapText(tester, '動画の添削が返ってきたとき');
    expect((api.requests.last as UpdateNotificationSettingsRequest).toJson(), {
      'video_review_returned': true,
    });
    expect(switchOf(tester, '動画の添削が返ってきたとき'), isTrue);
  });

  testWidgets('保存に失敗したらスイッチを元に戻し、失敗を知らせる', (tester) async {
    final api = FakeStudentApi(
      failures: {'updateMyNotificationSettings': Fixtures.unreachable},
    );
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openSettings(tester);

    await tapText(tester, '予約が確定したとき');

    expect(api.calls.last, 'updateMyNotificationSettings');
    expect(switchOf(tester, '予約が確定したとき'), isTrue);
    expect(
      container
          .read(studentFlowProvider)
          .notificationSettings!
          .bookingConfirmed,
      isTrue,
    );
    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    // 失敗のあとも切り替え直せる
    expect(
      tester
          .widget<SwitchListTile>(
            find.widgetWithText(SwitchListTile, '予約が確定したとき'),
          )
          .onChanged,
      isNotNull,
    );
  });

  testWidgets('読み込みに失敗したら知らせ、読み込み直せる', (tester) async {
    final failures = {'getMyNotificationSettings': Fixtures.unreachable};
    final api = FakeStudentApi(failures: failures);
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openSettings(tester);

    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    expect(find.byType(SwitchListTile), findsNothing);

    failures.clear();
    await tapText(tester, '設定を読み込み直す');

    expect(api.calls, [
      'getMyNotificationSettings',
      'getMyNotificationSettings',
    ]);
    expect(find.byType(SwitchListTile), findsNWidgets(3));
    expect(find.text('設定を読み込み直す'), findsNothing);
  });
}
