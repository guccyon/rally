import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/screens/live_lesson_screen.dart';
import 'package:rally_student_app/screens/prejoin_check_screen.dart';
import 'package:rally_student_app/state/prejoin_check.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/ui/components/level_meter.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';

import '../support/fake_local_media.dart';
import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final startsAt = Fixtures.slot.startsAt;

  /// 開始の 2 分 48 秒前（画面の仕様の代表の「あと 2:48」）
  final beforeStart = startsAt.subtract(
    const Duration(minutes: 2, seconds: 48),
  );

  StudentFlowState checking() => StudentFlowState(
    step: FlowStep.prejoinCheck,
    student: Fixtures.student,
    bookedSlot: Fixtures.slot,
    booking: Fixtures.bookingConfirmed,
  );

  Finder joinButton() => find.widgetWithText(FilledButton, '入室する');

  bool joinEnabled(WidgetTester tester) =>
      tester.widget<FilledButton>(joinButton()).onPressed != null;

  Future<
    ({FakeLocalMedia media, FakeStudentApi api, ProviderContainer container})
  >
  open(
    WidgetTester tester, {
    FakeLocalMedia? media,
    FakeStudentApi? api,
    DateTime Function()? clock,
  }) async {
    // スマートフォンの幅で、画面全体（入室の帯まで）が一度に描かれる高さにする
    tester.view.physicalSize = const Size(1170, 4200);
    tester.view.devicePixelRatio = 3;
    addTearDown(tester.view.reset);
    final m = media ?? FakeLocalMedia();
    final a = api ?? FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const PrejoinCheckScreen(),
      api: a,
      initial: checking(),
      media: m,
      clock: clock ?? () => beforeStart,
    );
    await tester.pump();
    return (media: m, api: a, container: container);
  }

  testWidgets('開くと許可を待つ表示を出し、許可すると自分の映り・切り替え・音の大きさを出す。開始時刻の前は入室できない', (
    tester,
  ) async {
    final media = FakeLocalMedia()..permissionGate = Completer<void>();
    await open(tester, media: media);

    expect(find.text('入室前チェック'), findsOneWidget);
    expect(find.text('カメラの許可を待っています'), findsOneWidget);
    expect(media.calls, ['openCamera']);

    media.permissionGate!.complete();
    await tester.pump();
    await tester.pump();

    expect(media.calls, ['openCamera', 'openMic']);
    expect(find.byKey(const ValueKey('fake-camera')), findsOneWidget);
    expect(find.bySemanticsLabel('自分のカメラの映り（左右反転）'), findsOneWidget);
    expect(find.text('マイクを切る'), findsOneWidget);
    expect(find.text('カメラを切る'), findsOneWidget);
    expect(find.text('まだ音が届いていません'), findsOneWidget);
    expect(find.text('スピーカーを試す'), findsOneWidget);
    // 予約の要約と入室の帯（開始時刻の前は押せない）
    expect(find.text('テニス'), findsOneWidget);
    expect(find.text('2:48'), findsOneWidget);
    expect(find.text('開始時刻になると入室できます'), findsOneWidget);
    expect(joinEnabled(tester), isFalse);
    // 端末の知らせは出さない
    expect(find.text('もう一度確かめる'), findsNothing);
  });

  testWidgets('開いたまま開始時刻になると、開き直さずに「入室する」が押せ、押すとカメラとマイクを止めて入・切を引き継ぐ', (
    tester,
  ) async {
    var now = startsAt.subtract(const Duration(seconds: 1));
    final r = await open(tester, clock: () => now);
    expect(joinEnabled(tester), isFalse);
    expect(find.text('0:01'), findsOneWidget);

    now = startsAt;
    await tester.pump(const Duration(seconds: 1));
    expect(joinEnabled(tester), isTrue);
    expect(find.text('開始時刻になりました。入室すると、コーチとのビデオ通話が始まります。'), findsOneWidget);

    await tapText(tester, 'マイクを切る');
    expect(find.text('マイクを切っています'), findsOneWidget);
    await tapText(tester, '入室する');

    expect(r.api.calls, ['startLiveLessonSession']);
    expect(r.api.requests.single, isA<StartLiveLessonSessionRequest>());
    final flow = r.container.read(studentFlowProvider);
    expect(flow.step, FlowStep.liveLesson);
    expect(flow.session!.status, LiveLessonSessionStatus.inProgress);
    final check = r.container.read(prejoinCheckProvider);
    expect(check.carried, (camera: true, mic: false));
    expect(r.media.calls, contains('stopAll'));
    expect(r.media.cameraOpen, isFalse);
    expect(r.media.micOpen, isFalse);
  });

  testWidgets('許可を断られたら理由と直し方を出し、切り替えは出さない。開始時刻からの入室は止めない', (tester) async {
    var now = beforeStart;
    final media = FakeLocalMedia(
      camera: MediaDeviceStatus.denied,
      mic: MediaDeviceStatus.denied,
    );
    final r = await open(tester, media: media, clock: () => now);

    expect(
      find.text(
        'カメラとマイクが使えません。端末の設定で許可すると確かめられます。\n'
        '許可しなくても、開始時刻から入室できます。そのときコーチには、あなたの映像と声が届きません。',
      ),
      findsOneWidget,
    );
    expect(find.text('カメラが使えません'), findsOneWidget);
    expect(
      find.text('マイクが使えないため、音の大きさを出せません。端末の設定で許可すると確かめられます。'),
      findsOneWidget,
    );
    expect(find.text('マイクを切る'), findsNothing);
    expect(find.text('カメラを切る'), findsNothing);
    // Web 版は端末の設定を開けないので、許可を変えたあとに確かめ直す
    expect(find.text('端末の設定を開く'), findsNothing);
    expect(find.text('もう一度確かめる'), findsOneWidget);
    expect(find.text('スピーカーを試す'), findsOneWidget);

    now = startsAt;
    await tester.pump(const Duration(seconds: 1));
    expect(joinEnabled(tester), isTrue);
    await tapText(tester, '入室する');
    final check = r.container.read(prejoinCheckProvider);
    expect(check.carried, (camera: false, mic: false));
  });

  testWidgets('片方だけ断られたら、断ったものだけを使えない表示にする。端末の設定を開ける端末は「端末の設定を開く」', (
    tester,
  ) async {
    final media = FakeLocalMedia(
      mic: MediaDeviceStatus.denied,
      canOpenSettings: true,
    );
    await open(tester, media: media);

    expect(
      find.textContaining('マイクが使えません。端末の設定で許可すると確かめられます。'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('fake-camera')), findsOneWidget);
    expect(find.text('カメラを切る'), findsOneWidget);
    expect(find.text('マイクを切る'), findsNothing);

    await tapText(tester, '端末の設定を開く');
    expect(media.calls.last, 'openSettings');
  });

  testWidgets('カメラやマイクがないときは知らせて、「もう一度確かめる」で見つかれば使い始めて知らせを消す', (tester) async {
    final media = FakeLocalMedia(camera: MediaDeviceStatus.missing);
    await open(tester, media: media);

    expect(
      find.text(
        'この端末にカメラが見つかりません。マイクとスピーカーは確かめられます。\n'
        'なくても、開始時刻から入室できます。',
      ),
      findsOneWidget,
    );
    expect(find.text('カメラが見つかりません'), findsOneWidget);
    expect(find.text('カメラを切る'), findsNothing);
    expect(find.text('マイクを切る'), findsOneWidget);

    media.camera = MediaDeviceStatus.ready;
    await tapText(tester, 'もう一度確かめる');

    expect(find.text('もう一度確かめる'), findsNothing);
    expect(find.byKey(const ValueKey('fake-camera')), findsOneWidget);
    expect(find.text('カメラを切る'), findsOneWidget);
    // マイクは使えているので開き直さない
    expect(media.calls.where((c) => c == 'openMic'), hasLength(1));
  });

  testWidgets('声を出すとバーが伸び、2 秒静かになると「まだ音が届いていません」に戻る。マイクを切るとマイクを止める', (
    tester,
  ) async {
    final r = await open(tester);

    r.media.emitLevel(0.6);
    await tester.pump();
    expect(find.text('音が届いています'), findsOneWidget);
    expect(tester.widget<LevelMeter>(find.byType(LevelMeter)).level, 6);

    for (var i = 0; i < 19; i++) {
      r.media.emitLevel(0);
    }
    await tester.pump();
    expect(find.text('音が届いています'), findsOneWidget);
    r.media.emitLevel(0);
    await tester.pump();
    expect(find.text('まだ音が届いていません'), findsOneWidget);

    await tapText(tester, 'マイクを切る');
    expect(r.media.calls.last, 'stopMic');
    expect(r.media.micOpen, isFalse);
    expect(find.text('マイクを切っています'), findsOneWidget);
    expect(tester.widget<LevelMeter>(find.byType(LevelMeter)).stopped, isTrue);

    await tapText(tester, 'マイクを入れる');
    expect(r.media.micOpen, isTrue);
    expect(find.text('まだ音が届いていません'), findsOneWidget);
  });

  testWidgets('カメラを切ると端末のカメラを止めて「カメラを切っています」、入れると映りが戻る', (tester) async {
    final r = await open(tester);

    await tapText(tester, 'カメラを切る');
    expect(r.media.calls.last, 'stopCamera');
    expect(r.media.cameraOpen, isFalse);
    expect(find.text('カメラを切っています'), findsOneWidget);
    expect(find.byKey(const ValueKey('fake-camera')), findsNothing);

    await tapText(tester, 'カメラを入れる');
    expect(find.byKey(const ValueKey('fake-camera')), findsOneWidget);
  });

  testWidgets('「スピーカーを試す」で試し音を 1 回鳴らし、鳴っているあいだは止められる。鳴り終わると元に戻る', (
    tester,
  ) async {
    final r = await open(tester);

    await tapText(tester, 'スピーカーを試す');
    expect(r.media.calls.last, 'playTestTone');
    expect(find.text('試し音を止める'), findsOneWidget);
    expect(find.text('試し音を鳴らしています。聞こえないときは端末の音量を上げてください。'), findsOneWidget);

    await tapText(tester, '試し音を止める');
    expect(r.media.calls.last, 'stopTestTone');
    expect(find.text('スピーカーを試す'), findsOneWidget);

    await tapText(tester, 'スピーカーを試す');
    r.media.finishTone();
    await tester.pumpAndSettle();
    expect(find.text('スピーカーを試す'), findsOneWidget);
    expect(find.text('イヤホンをつないでいれば、イヤホンから鳴ります。'), findsOneWidget);
  });

  testWidgets('試し音を鳴らせないときは直し方を出す', (tester) async {
    await open(tester, media: FakeLocalMedia(toneFails: true));

    await tapText(tester, 'スピーカーを試す');

    final error = find.text('試し音を鳴らせませんでした。端末の音量と、音の出る先を確かめてください。');
    expect(error, findsOneWidget);
    expect(tester.widget<Text>(error).style!.color, RallyColors.danger.fg);
    expect(find.text('スピーカーを試す'), findsOneWidget);
  });

  testWidgets('入室で 409 slot_not_started が返ったら、入室の帯に今の文言を出し、入室前チェックに留まる', (
    tester,
  ) async {
    final api = FakeStudentApi(
      failures: {
        'startLiveLessonSession': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.slotNotStarted,
            message: 'slot has not started',
          ),
        ),
      },
    );
    final r = await open(tester, api: api, clock: () => startsAt);

    await tapText(tester, '入室する');

    expect(
      find.text('この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。'),
      findsOneWidget,
    );
    final flow = r.container.read(studentFlowProvider);
    expect(flow.step, FlowStep.prejoinCheck);
    expect(flow.session, isNull);
    // カメラの映りは止めない
    expect(r.media.cameraOpen, isTrue);
  });

  testWidgets('戻るとカメラとマイクをすぐ止めて予約の確定に戻る', (tester) async {
    final r = await open(tester);
    expect(r.media.cameraOpen, isTrue);

    await tester.tap(find.byTooltip('予約に戻る'));
    await tester.pumpAndSettle();

    final flow = r.container.read(studentFlowProvider);
    expect(flow.step, FlowStep.bookingPending);
    expect(r.media.calls, contains('stopAll'));
    expect(r.media.cameraOpen, isFalse);
    expect(r.media.micOpen, isFalse);
  });

  testWidgets('アプリが裏に回るとカメラとマイクを止め、前に戻ると入にしていたものだけ開き直す', (tester) async {
    final r = await open(tester);
    await tapText(tester, 'カメラを切る');

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    await tester.pump();
    expect(r.media.cameraOpen, isFalse);
    expect(r.media.micOpen, isFalse);

    r.media.calls.clear();
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pump();
    await tester.pump();
    expect(r.media.calls, ['openMic']);
    expect(r.media.micOpen, isTrue);
    expect(find.text('カメラを切っています'), findsOneWidget);
  });

  testWidgets('画面を片付けるとカメラとマイクを止める', (tester) async {
    final r = await open(tester);
    expect(r.media.cameraOpen, isTrue);

    await tester.pumpWidget(const SizedBox());

    expect(r.media.cameraOpen, isFalse);
    expect(r.media.micOpen, isFalse);
  });

  testWidgets('入室したあとのビデオ通話の画面に、入室前チェックで決めたカメラ・マイクの入・切を 1 行出す', (tester) async {
    await pumpScreen(
      tester,
      const LiveLessonScreen(),
      api: FakeStudentApi(),
      initial: checking().copyWith(
        step: FlowStep.liveLesson,
        session: Fixtures.sessionInProgress,
      ),
      prejoin: const PrejoinCheckState(carried: (camera: true, mic: false)),
    );

    expect(find.text('カメラ: 入・マイク: 切（入室前チェックのまま）'), findsOneWidget);
    expect(find.text('通話を開始する'), findsNothing);
    expect(find.text('通話を終了する'), findsOneWidget);
  });
}
