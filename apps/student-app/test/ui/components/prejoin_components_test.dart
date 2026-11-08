import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/components/camera_preview_frame.dart';
import 'package:rally_student_app/ui/components/join_bar.dart';
import 'package:rally_student_app/ui/components/level_meter.dart';
import 'package:rally_student_app/ui/components/media_toggle.dart';
import 'package:rally_student_app/ui/countdown.dart';
import 'package:rally_student_app/ui/theme/rally_theme.dart';
import 'package:rally_student_app/ui/theme/rally_tokens.dart';

/// 入室前チェックの部品（components.yaml の media_toggle・camera_preview・level_meter・join_bar。0.7.0）と
/// 残り時間の表記（content.yaml の notation.datetime.countdown）。
Future<void> pumpPart(WidgetTester tester, Widget child) => tester.pumpWidget(
  MaterialApp(
    theme: buildRallyTheme(),
    home: Scaffold(
      body: ListView(padding: const EdgeInsets.all(16), children: [child]),
    ),
  ),
);

Color? _fill(WidgetTester tester, Finder finder) =>
    (tester.widget<Container>(finder).decoration as BoxDecoration?)?.color;

/// 枠の面の色
Color _frame(WidgetTester tester) => tester
    .widget<ColoredBox>(
      find.descendant(
        of: find.byType(AspectRatio),
        matching: find.byType(ColoredBox),
      ),
    )
    .color;

void main() {
  group('入・切の切り替え', () {
    testWidgets('押すとその場で文言とアイコンを入れ替え、面は secondary のまま', (tester) async {
      var on = true;
      await pumpPart(
        tester,
        StatefulBuilder(
          builder: (context, setState) => MediaToggle(
            kind: MediaToggleKind.mic,
            on: on,
            onToggle: () => setState(() => on = !on),
          ),
        ),
      );
      expect(find.text('マイクを切る'), findsOneWidget);
      expect(find.byIcon(Icons.mic_rounded), findsOneWidget);
      final before = tester.getSize(find.byType(FilledButton));
      expect(before.height, greaterThanOrEqualTo(56));

      await tester.tap(find.text('マイクを切る'));
      await tester.pump();
      expect(on, isFalse);
      expect(find.text('マイクを入れる'), findsOneWidget);
      expect(find.byIcon(Icons.mic_off_rounded), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    test('文言は content.yaml の動詞', () {
      expect(mediaToggleLabel(MediaToggleKind.mic, on: true), 'マイクを切る');
      expect(mediaToggleLabel(MediaToggleKind.mic, on: false), 'マイクを入れる');
      expect(mediaToggleLabel(MediaToggleKind.camera, on: true), 'カメラを切る');
      expect(mediaToggleLabel(MediaToggleKind.camera, on: false), 'カメラを入れる');
      expect(
        mediaToggleLabel(MediaToggleKind.speakerTest, on: true),
        '試し音を止める',
      );
      expect(
        mediaToggleLabel(MediaToggleKind.speakerTest, on: false),
        'スピーカーを試す',
      );
    });

    testWidgets('許可を求めているあいだと、ない・許可がないときは押せない', (tester) async {
      var pressed = 0;
      await pumpPart(
        tester,
        Column(
          children: [
            MediaToggle(
              kind: MediaToggleKind.camera,
              on: true,
              loading: true,
              onToggle: () => pressed++,
            ),
            MediaToggle(
              kind: MediaToggleKind.mic,
              on: false,
              disabled: true,
              onToggle: () => pressed++,
            ),
          ],
        ),
      );
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      await tester.tap(find.text('カメラを切る'));
      await tester.tap(find.text('マイクを入れる'));
      expect(pressed, 0);
    });
  });

  group('カメラの映りの枠', () {
    testWidgets('映っているときは左右反転のラベルの画像で、4:3', (tester) async {
      final handle = tester.ensureSemantics();
      await pumpPart(
        tester,
        const RallyCameraPreview(
          state: CameraPreviewState.live,
          video: SizedBox(width: 640, height: 480),
        ),
      );
      expect(find.bySemanticsLabel('自分のカメラの映り（左右反転）'), findsOneWidget);
      final size = tester.getSize(find.byType(AspectRatio));
      expect(size.width / size.height, closeTo(4 / 3, 0.01));
      expect(
        tester
            .widget<Transform>(
              find.descendant(
                of: find.byType(AspectRatio),
                matching: find.byType(Transform),
              ),
            )
            .transform
            .storage[0],
        -1,
      );
      handle.dispose();
    });

    testWidgets('許可待ちは濃い面、切・使えない・ないは surface の面に videocam_off と文', (
      tester,
    ) async {
      await pumpPart(
        tester,
        const RallyCameraPreview(state: CameraPreviewState.waiting),
      );
      expect(find.text('カメラの許可を待っています'), findsOneWidget);
      expect(find.byIcon(Icons.videocam_off_rounded), findsNothing);
      expect(
        _frame(tester),
        RallyColors.inverseSurface,
      );

      await pumpPart(
        tester,
        const RallyCameraPreview(state: CameraPreviewState.denied),
      );
      expect(find.text('カメラが使えません'), findsOneWidget);
      expect(find.text('端末の設定で許可すると確かめられます。入室は開始時刻からできます。'), findsOneWidget);
      expect(find.byIcon(Icons.videocam_off_rounded), findsOneWidget);
      expect(
        tester.widget<Icon>(find.byIcon(Icons.videocam_off_rounded)).size,
        24,
      );
      expect(
        _frame(tester),
        RallyColors.surface,
      );
    });
  });

  group('音の大きさ', () {
    testWidgets('10 段で届いた段まで primary、残りは primary_container。止めているときは全部 surface', (
      tester,
    ) async {
      await pumpPart(tester, const LevelMeter(level: 6, label: '音が届いています'));
      expect(find.text('音が届いています'), findsOneWidget);
      for (var i = 0; i < LevelMeter.segments; i++) {
        final segment = find.byKey(ValueKey('level-segment-$i'));
        expect(
          _fill(tester, segment),
          i < 6 ? RallyColors.primary : RallyColors.primaryContainer,
        );
        expect(tester.getSize(segment).height, 8);
      }

      await pumpPart(
        tester,
        const LevelMeter(level: 6, label: 'マイクを切っています', stopped: true),
      );
      for (var i = 0; i < LevelMeter.segments; i++) {
        expect(
          _fill(tester, find.byKey(ValueKey('level-segment-$i'))),
          RallyColors.surface,
        );
      }
    });
  });

  group('残り時間', () {
    test('1 時間未満だけ「分:秒」で、読み上げは分と秒の文', () {
      expect(formatCountdown(const Duration(minutes: 2, seconds: 48)), '2:48');
      expect(formatCountdown(const Duration(seconds: 65)), '1:05');
      expect(formatCountdown(const Duration(hours: 1)), isNull);
      expect(formatCountdown(Duration.zero), isNull);
      expect(
        countdownReadAloud(const Duration(minutes: 2, seconds: 48)),
        '開始まで あと 2 分 48 秒',
      );
      expect(countdownReadAloud(const Duration(minutes: 2)), '開始まで あと 2 分');
      expect(countdownReadAloud(Duration.zero), '開始時刻になりました。入室できます');
    });

    test('読み上げの文は 1 分ごとと 0 になったときだけ変わる', () {
      expect(
        countdownAnnouncement(const Duration(minutes: 2, seconds: 48)),
        '開始まで あと 3 分',
      );
      expect(
        countdownAnnouncement(const Duration(minutes: 2)),
        '開始まで あと 2 分',
      );
      expect(countdownAnnouncement(Duration.zero), '開始時刻になりました。入室できます');
      expect(countdownAnnouncement(const Duration(hours: 2)), '');
    });
  });

  group('入室の帯', () {
    testWidgets('開始時刻の前は「入室する」を押せず、理由と残り時間を出す。開始時刻になったら押せる', (
      tester,
    ) async {
      var joined = 0;
      Widget bar(Duration remaining) => MaterialApp(
        theme: buildRallyTheme(),
        home: Scaffold(
          body: const SizedBox.shrink(),
          bottomNavigationBar: JoinBar(
            startTime: '12:00',
            remaining: remaining,
            onJoin: () => joined++,
          ),
        ),
      );
      await tester.pumpWidget(bar(const Duration(minutes: 2, seconds: 48)));
      expect(find.text('2:48'), findsOneWidget);
      expect(find.text('開始時刻になると入室できます'), findsOneWidget);
      await tester.tap(find.text('入室する'));
      expect(joined, 0);

      await tester.pumpWidget(bar(Duration.zero));
      expect(find.text('2:48'), findsNothing);
      expect(find.text('カメラとマイクの入・切は、そのまま通話に引き継ぎます'), findsOneWidget);
      await tester.tap(find.text('入室する'));
      expect(joined, 1);
      // 「入室する」は幅いっぱい
      expect(
        tester.getSize(find.byType(FilledButton)).width,
        800 - RallySpacing.md * 2,
      );
    });

    testWidgets('入室に失敗したらボタンの上に失敗の知らせを出し、ボタンは押せるまま', (tester) async {
      var joined = 0;
      await pumpPart(
        tester,
        JoinBar(
          startTime: '12:00',
          remaining: Duration.zero,
          error: 'この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。',
          onJoin: () => joined++,
        ),
      );
      expect(find.textContaining('ビデオ通話を始められません'), findsOneWidget);
      await tester.tap(find.text('入室する'));
      expect(joined, 1);
    });
  });
}
