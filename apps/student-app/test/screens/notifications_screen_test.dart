import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/notifications.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/ui/labels.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final home = StudentFlowState(
    step: FlowStep.home,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  ApiNotification notification(
    String id, {
    required NotificationKind? kind,
    required String body,
    required int minutesAgo,
    bool isRead = false,
  }) => ApiNotification(
    notificationId: id,
    kind: kind,
    body: body,
    createdAt: Fixtures.now.subtract(Duration(minutes: minutesAgo)),
    isRead: isRead,
  );

  final booking = notification(
    'n-booking',
    kind: NotificationKind.bookingConfirmed,
    body: '予約が確定しました',
    minutesAgo: 1,
  );
  final review = notification(
    'n-review',
    kind: NotificationKind.videoReviewReturned,
    body: '動画の添削が返ってきました',
    minutesAgo: 2,
  );
  final message = notification(
    'n-message',
    kind: NotificationKind.messageReceived,
    body: 'コーチからメッセージが届きました',
    minutesAgo: 3,
    isRead: true,
  );

  /// このアプリの版が知らない種類（`kind` が null）のお知らせ
  final unknown = notification(
    'n-unknown',
    kind: null,
    body: '新しい機能のお知らせです',
    minutesAgo: 4,
  );

  /// ホームを描いて欄を読む（登録の直後と同じ）
  Future<ProviderContainer> pumpHome(
    WidgetTester tester,
    FakeStudentApi api,
  ) async {
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
    );
    await container.read(studentFlowProvider.notifier).refreshHome();
    await tester.pumpAndSettle();
    return container;
  }

  /// アプリバーの「お知らせ」の点のしるしが出ているか
  bool badgeVisible(WidgetTester tester) => tester
      .widget<Badge>(
        find.descendant(
          of: find.byTooltip('お知らせ'),
          matching: find.byType(Badge),
        ),
      )
      .isLabelVisible;

  Future<void> openNotifications(WidgetTester tester) async {
    await tester.tap(find.byTooltip('お知らせ'));
    await tester.pumpAndSettle();
  }

  Finder tileOf(ApiNotification n) =>
      find.ancestor(of: find.text(n.body), matching: find.byType(ListTile));

  group('ホームのしるし', () {
    testWidgets('未読のお知らせがあれば、ホームのアプリバーの「お知らせ」に点のしるしを付ける', (tester) async {
      final api = FakeStudentApi(notifications: [booking, message]);
      await pumpHome(tester, api);

      expect(api.calls, contains('listMyNotifications'));
      expect(badgeVisible(tester), isTrue);
    });

    testWidgets('すべて既読か 0 件なら、しるしを付けない', (tester) async {
      await pumpHome(tester, FakeStudentApi(notifications: [message]));
      expect(badgeVisible(tester), isFalse);
    });

    testWidgets('お知らせを読めなくても、ホームの欄はそのまま出してしるしを付けないだけにする', (tester) async {
      final api = FakeStudentApi(
        notifications: [booking],
        failures: {'listMyNotifications': Fixtures.unreachable},
      );
      final container = await pumpHome(tester, api);

      expect(badgeVisible(tester), isFalse);
      expect(container.read(studentFlowProvider).homeFailures, isEmpty);
      expect(container.read(studentFlowProvider).errorMessage, isNull);
    });

    testWidgets('ほかの画面では、未読があってもしるしを付けない', (tester) async {
      await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: FakeStudentApi(),
        initial: home.copyWith(step: FlowStep.slotSearch),
        notifications: NotificationsState(notifications: [booking]),
      );
      await tester.pumpAndSettle();

      expect(badgeVisible(tester), isFalse);
    });
  });

  group('お知らせの画面', () {
    testWidgets('しるしを押すと開き、新しい順に本文と作った時刻を出し、開いたときに未読だったものに「新着」と添える', (
      tester,
    ) async {
      final api = FakeStudentApi(notifications: [booking, review, message]);
      final container = await pumpHome(tester, api);
      api.calls.clear();

      await openNotifications(tester);

      expect(container.read(studentFlowProvider).step, FlowStep.notifications);
      expect(find.text('お知らせ'), findsOneWidget);
      final bodies = [
        for (final tile in tester.widgetList<ListTile>(
          find.descendant(
            of: find.byType(Card),
            matching: find.byType(ListTile),
          ),
        ))
          (tile.title! as Text).data!,
      ];
      expect(bodies, [booking.body, review.body, message.body]);
      expect(
        find.text('新着・${formatDateTime(booking.createdAt)}'),
        findsOneWidget,
      );
      expect(
        find.text('新着・${formatDateTime(review.createdAt)}'),
        findsOneWidget,
      );
      expect(find.text(formatDateTime(message.createdAt)), findsOneWidget);
    });

    testWidgets('開いたらすべて既読にし、閉じてホームに戻るとしるしが消える', (tester) async {
      final api = FakeStudentApi(notifications: [booking, message]);
      final container = await pumpHome(tester, api);
      expect(badgeVisible(tester), isTrue);
      api.calls.clear();

      await openNotifications(tester);

      expect(api.calls, ['listMyNotifications', 'markMyNotificationsRead']);
      expect(api.notifications.every((n) => n.isRead), isTrue);
      expect(container.read(notificationsProvider).hasUnread, isFalse);
      // 既読にしたあとも、開いたときに未読だったものは新着として見分ける
      expect(
        find.text('新着・${formatDateTime(booking.createdAt)}'),
        findsOneWidget,
      );

      await tester.tap(find.byTooltip('お知らせを閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.home);
      expect(badgeVisible(tester), isFalse);
    });

    testWidgets('未読がなければ既読にする操作は呼ばない', (tester) async {
      final api = FakeStudentApi(notifications: [message]);
      await pumpHome(tester, api);
      api.calls.clear();

      await openNotifications(tester);

      expect(api.calls, ['listMyNotifications']);
    });

    testWidgets('既読にできなくても一覧は出し、失敗の文言は出さず、しるしは残す', (tester) async {
      final api = FakeStudentApi(
        notifications: [booking],
        failures: {'markMyNotificationsRead': Fixtures.unreachable},
      );
      final container = await pumpHome(tester, api);

      await openNotifications(tester);

      expect(find.text(booking.body), findsOneWidget);
      expect(container.read(studentFlowProvider).errorMessage, isNull);
      expect(container.read(notificationsProvider).hasUnread, isTrue);

      await tester.tap(find.byTooltip('お知らせを閉じる'));
      await tester.pumpAndSettle();
      expect(badgeVisible(tester), isTrue);
    });

    testWidgets('読み込み中はその旨を出す', (tester) async {
      final gate = Completer<void>();
      final api = FakeStudentApi(notifications: [booking])
        ..notificationsGate = gate;
      await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: home.copyWith(step: FlowStep.slotSearch),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('お知らせ'));
      await tester.pump();
      expect(find.text('お知らせを読み込んでいます…'), findsOneWidget);

      gate.complete();
      await tester.pumpAndSettle();
      expect(find.text('お知らせを読み込んでいます…'), findsNothing);
      expect(find.text(booking.body), findsOneWidget);
    });

    testWidgets('0 件ならその旨を出す', (tester) async {
      await pumpHome(tester, FakeStudentApi());
      await openNotifications(tester);

      expect(
        find.text('まだお知らせはありません。予約の確定・添削の返却・コーチからのメッセージがここに届きます。'),
        findsOneWidget,
      );
    });

    testWidgets('読めなければその旨と「もう一度読み込む」を出し（失敗の文言は出さない）、押すと読み直す', (tester) async {
      final failures = <String, StudentApiException>{
        'listMyNotifications': Fixtures.unreachable,
      };
      final api = FakeStudentApi(notifications: [booking], failures: failures);
      final container = await pumpHome(tester, api);

      await openNotifications(tester);

      expect(find.text('お知らせを読み込めませんでした。'), findsOneWidget);
      expect(container.read(studentFlowProvider).errorMessage, isNull);

      failures.clear();
      await tapText(tester, 'もう一度読み込む');
      expect(find.text('お知らせを読み込めませんでした。'), findsNothing);
      expect(find.text(booking.body), findsOneWidget);
    });

    testWidgets('ほかの画面から開くと、閉じたときに開いた画面に戻る', (tester) async {
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: FakeStudentApi(notifications: [booking]),
        initial: home.copyWith(step: FlowStep.slotSearch),
      );
      await tester.pumpAndSettle();

      await openNotifications(tester);
      expect(container.read(studentFlowProvider).step, FlowStep.notifications);
      // お知らせの画面からはアプリバーでほかの脇の画面を開かない
      expect(find.byTooltip('お知らせ'), findsNothing);

      await tester.tap(find.byTooltip('お知らせを閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.slotSearch);
    });
  });

  group('行から進む', () {
    testWidgets('予約の確定は自分の予約を開き、閉じるとお知らせに戻る', (tester) async {
      final api = FakeStudentApi(notifications: [booking]);
      final container = await pumpHome(tester, api);
      await openNotifications(tester);
      api.calls.clear();

      await tester.tap(tileOf(booking));
      await tester.pumpAndSettle();

      expect(api.calls, ['listMyBookings']);
      expect(container.read(studentFlowProvider).step, FlowStep.myBookings);

      await tester.tap(find.byTooltip('自分の予約を閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.notifications);
      expect(find.text(booking.body), findsOneWidget);

      await tester.tap(find.byTooltip('お知らせを閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.home);
    });

    testWidgets('添削の返却は自分の動画（添削結果の入口）を開き、閉じるとお知らせに戻る', (tester) async {
      final api = FakeStudentApi(notifications: [review]);
      final container = await pumpHome(tester, api);
      await openNotifications(tester);
      api.calls.clear();

      await tester.tap(tileOf(review));
      await tester.pumpAndSettle();

      expect(api.calls, ['listMyVideos']);
      expect(container.read(studentFlowProvider).step, FlowStep.myVideos);

      await tester.tap(find.byTooltip('自分の動画を閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.notifications);
    });

    testWidgets('メッセージの受信はチャットのスレッドの一覧を開き、閉じるとお知らせに戻る', (tester) async {
      final api = FakeStudentApi(notifications: [message]);
      final container = await pumpHome(tester, api);
      await openNotifications(tester);
      api.calls.clear();

      await tester.tap(tileOf(message));
      await tester.pumpAndSettle();

      expect(api.calls, ['listMessageThreads']);
      expect(container.read(studentFlowProvider).step, FlowStep.messageThreads);
      expect(container.read(chatProvider).returnStep, FlowStep.notifications);

      await tester.tap(find.byTooltip('チャットを閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.notifications);
    });

    testWidgets('知らない種類のお知らせは本文を出すが、行を押せない', (tester) async {
      final api = FakeStudentApi(notifications: [unknown]);
      final container = await pumpHome(tester, api);
      await openNotifications(tester);
      api.calls.clear();

      expect(find.text(unknown.body), findsOneWidget);
      expect(tester.widget<ListTile>(tileOf(unknown)).onTap, isNull);
      await tester.tap(tileOf(unknown));
      await tester.pumpAndSettle();

      expect(api.calls, isEmpty);
      expect(container.read(studentFlowProvider).step, FlowStep.notifications);
    });
  });
}
