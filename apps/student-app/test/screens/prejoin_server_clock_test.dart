import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/server_clock.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/prejoin_check.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

/// 端末の時計がずれていても、予約の確定の「ビデオ通話で受ける」と入室前チェックの残り時間・「入室する」を、
/// 応答の Date から出したずれで補正した時計（サーバーの時刻）で判断する（specs/ux/issue-1040.yaml の open_questions の 2）。
void main() {
  final startsAt = Fixtures.slot.startsAt;
  final opensAt = prejoinCheckOpensAt(startsAt);

  StudentFlowState confirmed() => StudentFlowState(
    step: FlowStep.bookingPending,
    student: Fixtures.student,
    bookedSlot: Fixtures.slot,
    booking: Fixtures.bookingConfirmed,
  );

  const slotNotStarted = StudentApiException(
    statusCode: 409,
    error: ApiError(
      code: ErrorCode.slotNotStarted,
      message: 'slot has not started',
    ),
  );

  Finder videoCallButton() => find.widgetWithText(FilledButton, 'ビデオ通話で受ける');
  Finder joinButton() => find.widgetWithText(FilledButton, '入室する');
  bool enabled(WidgetTester tester, Finder button) =>
      tester.widget<FilledButton>(button).onPressed != null;

  /// 予約の確定から、端末の時計を [deviceNow] にして描き、ホームの欄を読む（`listMyBookings` の応答でずれを決める）
  Future<ProviderContainer> open(
    WidgetTester tester, {
    required FakeStudentApi api,
    required DateTime Function() deviceNow,
  }) async {
    // スマートフォンの幅で、画面全体（入室の帯まで）が一度に描かれる高さにする
    tester.view.physicalSize = const Size(1170, 4200);
    tester.view.devicePixelRatio = 3;
    addTearDown(tester.view.reset);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: confirmed(),
      serverClock: ServerClock(deviceNow),
    );
    await container.read(studentFlowProvider.notifier).refreshHome();
    await tester.pump();
    return container;
  }

  for (final (label, skew) in [
    ('2 分遅れ', const Duration(minutes: -2)),
    ('2 分進み', const Duration(minutes: 2)),
  ]) {
    testWidgets('端末の時計が$labelでも、サーバーの時刻で開始 5 分前に入室前チェックを開け、開始時刻に入室できる', (
      tester,
    ) async {
      var server = opensAt.subtract(const Duration(seconds: 1));
      final api = FakeStudentApi()..clockOffset = -skew;
      final container = await open(
        tester,
        api: api,
        deviceNow: () => server.add(skew),
      );
      expect(api.calls, contains('listMyBookings'));

      // サーバーの時刻で開始 5 分前の 1 秒前は、まだ開けない
      expect(enabled(tester, videoCallButton()), isFalse);
      server = opensAt;
      await tester.pump(const Duration(seconds: 1));
      expect(enabled(tester, videoCallButton()), isTrue);

      await tapText(tester, 'ビデオ通話で受ける');
      expect(container.read(studentFlowProvider).step, FlowStep.prejoinCheck);
      expect(find.text('入室前チェック'), findsOneWidget);
      // 残り時間もサーバーの時刻で数える
      expect(find.text('5:00'), findsOneWidget);
      expect(enabled(tester, joinButton()), isFalse);

      server = startsAt.subtract(const Duration(seconds: 1));
      await tester.pump(const Duration(seconds: 1));
      expect(find.text('0:01'), findsOneWidget);
      expect(enabled(tester, joinButton()), isFalse);

      server = startsAt;
      await tester.pump(const Duration(seconds: 1));
      expect(enabled(tester, joinButton()), isTrue);
      await tapText(tester, '入室する');
      expect(container.read(studentFlowProvider).step, FlowStep.liveLesson);
      expect(
        api.calls.where((c) => c == 'startLiveLessonSession'),
        hasLength(1),
      );
    });
  }

  testWidgets(
    '入室が 409 slot_not_started なら、その応答のずれで合わせ直して残り時間の表示に戻り、入室を繰り返し送らない',
    (tester) async {
      // 端末の時計は 2 分進んでいるが、予約の一覧の応答には Date がなかった（端末の時計のまま）
      var server = startsAt.subtract(const Duration(minutes: 2));
      const skew = Duration(minutes: 2);
      final api = FakeStudentApi(
        failures: {'startLiveLessonSession': slotNotStarted},
      );
      final container = await open(
        tester,
        api: api,
        deviceNow: () => server.add(skew),
      );
      await tapText(tester, 'ビデオ通話で受ける');
      expect(enabled(tester, joinButton()), isTrue);

      // 入室の 409 の応答には Date がある
      api.clockOffset = -skew;
      await tapText(tester, '入室する');

      expect(container.read(studentFlowProvider).step, FlowStep.prejoinCheck);
      expect(
        find.text('この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。'),
        findsOneWidget,
      );
      expect(find.text('2:00'), findsOneWidget);
      expect(enabled(tester, joinButton()), isFalse);

      // 待っていても入室は送り直さない。サーバーの開始時刻になったら押せる
      server = startsAt.subtract(const Duration(seconds: 1));
      await tester.pump(const Duration(seconds: 1));
      expect(find.text('0:01'), findsOneWidget);
      expect(enabled(tester, joinButton()), isFalse);
      server = startsAt;
      await tester.pump(const Duration(seconds: 1));
      expect(enabled(tester, joinButton()), isTrue);
      expect(
        api.calls.where((c) => c == 'startLiveLessonSession'),
        hasLength(1),
      );
    },
  );

  testWidgets('応答に Date がないときは、端末の時計のままで開け、開始時刻に入室できる', (tester) async {
    var device = opensAt.subtract(const Duration(seconds: 1));
    final api = FakeStudentApi();
    final container = await open(tester, api: api, deviceNow: () => device);
    expect(container.read(serverClockProvider).offset, Duration.zero);

    expect(enabled(tester, videoCallButton()), isFalse);
    device = opensAt;
    await tester.pump(const Duration(seconds: 1));
    await tapText(tester, 'ビデオ通話で受ける');
    expect(find.text('5:00'), findsOneWidget);

    device = startsAt;
    await tester.pump(const Duration(seconds: 1));
    await tapText(tester, '入室する');
    expect(container.read(studentFlowProvider).step, FlowStep.liveLesson);
  });
}
