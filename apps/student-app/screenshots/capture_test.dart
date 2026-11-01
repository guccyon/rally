// 画面のキャプチャ。テストではなく、PR の前後で見た目を比べるための撮影。
//
//   SCREENSHOT_DIR=screenshots/out flutter test screenshots/
//
// SCREENSHOT_DIR がないときは何もしない（`flutter test` の既定の対象 test/ にも入れていない）。
// 画面ごとに <id>.png と <id>.json（題名・取れたか）を書き、比較の道具（tools/screenshots）が読む。
// アプリ全体（StudentApp）を API のモック（test/support の FakeStudentApi）で動かし、画面を押してたどる。
// 画面を足したり、たどり方（押す文言）を変えたりしたら、下の screens を直す。
// id はファイル名と前後の突き合わせに使うので変えない（変えると「消えた画面」と「新しい画面」になる）。
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/screens/design_catalog_screen.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../test/support/fake_student_api.dart';
import '../test/support/pump.dart';

typedef Navigate = Future<void> Function(WidgetTester tester);

class Screen {
  const Screen(this.id, this.title, this.navigate, {this.home});
  final String id;
  final String title;
  final Navigate navigate;

  /// 最初の画面（なければアプリの画面の流れ）
  final Widget? home;
}

Future<void> _register(WidgetTester tester) async {
  await tester.enterText(
    find.widgetWithText(TextFormField, 'メールアドレス'),
    'yamada@example.com',
  );
  await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
  await tapText(tester, 'テニス');
  await tapText(tester, '登録する');
}

Future<void> _tapTooltip(WidgetTester tester, String tooltip) async {
  await tester.tap(find.byTooltip(tooltip));
  await tester.pumpAndSettle();
}

/// 縦に長い画面を、画面の高さを伸ばして下まで 1 枚に収める。
Future<void> _showWholePage(WidgetTester tester) async {
  final position = tester
      .state<ScrollableState>(find.byType(Scrollable).first)
      .position;
  final ratio = tester.view.devicePixelRatio;
  final size = tester.view.physicalSize / ratio;
  tester.view.physicalSize =
      Size(size.width, size.height + position.maxScrollExtent) * ratio;
  await tester.pumpAndSettle();
}

Future<void> _toBooking(WidgetTester tester) async {
  await _register(tester);
  await tapText(tester, 'スロットを探して予約する');
  await tapText(tester, '体験予約する');
}

Future<void> _toConfirmed(WidgetTester tester) async {
  await _toBooking(tester);
  await tapText(tester, '予約を確定する');
}

Future<void> _toCompletion(WidgetTester tester) async {
  await _toConfirmed(tester);
  await tapText(tester, 'ビデオ通話で受ける');
  await tapText(tester, '通話を開始する');
  await tapText(tester, '通話を終了する');
}

Future<void> _toReview(WidgetTester tester) async {
  await _toCompletion(tester);
  await tapText(tester, '完了を記録する');
  await tapText(tester, 'レッスンを評価する');
}

final screens = <Screen>[
  Screen('register', 'アカウント登録', (_) async {}),
  // デザインシステムの部品のカタログ（アプリの流れからは開けない）。部品が変わるとここに出る
  Screen(
    'design-catalog',
    'デザインシステム: 部品のカタログ',
    _showWholePage,
    home: const DesignCatalogScreen(),
  ),
  Screen('home', 'ホーム', _register),
  Screen('coach-search', 'コーチを探す', (t) async {
    await _register(t);
    await tapText(t, 'コーチを探す');
  }),
  Screen('coach-profile', 'コーチの紹介', (t) async {
    await _register(t);
    await tapText(t, 'コーチを探す');
    await tapText(t, Fixtures.coachListings.first.displayName);
  }),
  Screen('slot-search', 'スロットを探す', (t) async {
    await _register(t);
    await tapText(t, 'スロットを探して予約する');
  }),
  Screen('slot-search-coach', 'スロットを探す: コーチで絞る', (t) async {
    await _register(t);
    await tapText(t, 'コーチを探す');
    await tapText(t, Fixtures.coachListings.first.displayName);
    await tapText(t, 'このコーチの空き枠を見る');
  }),
  Screen('booking-pending', '予約の確定待ち', _toBooking),
  Screen('booking-confirmed', '予約の確定', _toConfirmed),
  Screen('live-lesson', 'ビデオ通話', (t) async {
    await _toConfirmed(t);
    await tapText(t, 'ビデオ通話で受ける');
  }),
  Screen('video-submit', '動画を送る', (t) async {
    await _toConfirmed(t);
    await tapText(t, '動画を送って添削を受ける');
  }),
  Screen('lesson-completion', 'レッスンの完了', _toCompletion),
  Screen('lesson-review', 'レッスンの評価', _toReview),
  Screen('subscription', '月額契約', (t) async {
    await _toReview(t);
    await tapText(t, '月額契約へ進む');
  }),
  Screen('message-threads', 'チャット: スレッドの一覧', (t) async {
    await _register(t);
    await _tapTooltip(t, 'チャット');
  }),
  Screen('messages', 'チャット: メッセージ', (t) async {
    await _register(t);
    await _tapTooltip(t, 'チャット');
    await tapText(t, '担当コーチ');
  }),
  Screen('notifications', 'お知らせ', (t) async {
    await _register(t);
    await _tapTooltip(t, 'お知らせ');
  }),
  Screen('notification-settings', '通知の受け取り設定', (t) async {
    await _register(t);
    await _tapTooltip(t, '通知の設定');
  }),
  Screen('profile', 'プロフィール', (t) async {
    await _register(t);
    await _tapTooltip(t, 'プロフィール');
  }),
  Screen('my-bookings', '自分の予約', (t) async {
    await _register(t);
    await _tapTooltip(t, '自分の予約');
  }),
  Screen('my-videos', '自分の動画', (t) async {
    await _register(t);
    await _tapTooltip(t, '自分の動画');
  }),
];

/// キャプチャ用の API のモック。既定の見本に、受講権とお知らせとお気に入りのコーチを足して画面を埋める。
FakeStudentApi _api() => FakeStudentApi(
  entitlement: Fixtures.subscriptionStarted.entitlement,
  favoriteCoaches: [Fixtures.favoriteCoach()],
  notifications: [
    ApiNotification(
      notificationId: '99999999-9999-4999-8999-999999999991',
      kind: NotificationKind.bookingConfirmed,
      body: '10/28（水）12:00 の予約が確定しました。',
      createdAt: Fixtures.now.subtract(const Duration(hours: 2)),
      isRead: false,
    ),
    ApiNotification(
      notificationId: '99999999-9999-4999-8999-999999999992',
      kind: NotificationKind.videoReviewReturned,
      body: '佐藤 コーチが動画の添削を返しました。',
      createdAt: Fixtures.now.subtract(const Duration(days: 1)),
      isRead: true,
    ),
  ],
);

/// 撮った画像を SCREENSHOT_DIR に書くだけの比較役（matchesGoldenFile の描画をそのまま使う）。
class _WriteComparator extends GoldenFileComparator {
  _WriteComparator(this.dir);
  final Directory dir;

  @override
  Future<bool> compare(Uint8List imageBytes, Uri golden) async {
    await update(golden, imageBytes);
    return true;
  }

  @override
  Future<void> update(Uri golden, Uint8List imageBytes) async {
    await File('${dir.path}/${golden.path}').writeAsBytes(imageBytes);
  }
}

/// 日本語が四角にならないよう、日本語のフォントを既定の書体（Roboto）として読み込む。
/// アイコンは Flutter の SDK に入っている Material Icons を読み込む。
Future<void> _loadFonts() async {
  final candidates = [
    Platform.environment['SCREENSHOT_FONT'],
    '/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf',
    '/usr/share/fonts/truetype/fonts-japanese-gothic.ttf',
  ];
  final text = candidates
      .whereType<String>()
      .map(File.new)
      .where((f) => f.existsSync())
      .firstOrNull;
  if (text != null) {
    final loader = FontLoader('Roboto')
      ..addFont(Future.value(ByteData.sublistView(text.readAsBytesSync())));
    await loader.load();
  }
  final flutterRoot = Platform.environment['FLUTTER_ROOT'];
  final icons = File(
    '$flutterRoot/bin/cache/artifacts/material_fonts/MaterialIcons-Regular.otf',
  );
  if (flutterRoot != null && icons.existsSync()) {
    final loader = FontLoader('MaterialIcons')
      ..addFont(Future.value(ByteData.sublistView(icons.readAsBytesSync())));
    await loader.load();
  }
}

void main() {
  final outPath = Platform.environment['SCREENSHOT_DIR'];
  final skip = outPath == null ? 'SCREENSHOT_DIR がないので撮らない' : null;
  final out = Directory(outPath ?? '.');

  setUpAll(() async {
    if (skip != null) return;
    out.createSync(recursive: true);
    // 右上の「DEBUG」の帯を出さない（アプリ側は消していないため）
    WidgetsApp.debugAllowBannerOverride = false;
    goldenFileComparator = _WriteComparator(out);
    await _loadFonts();
  });

  for (final screen in screens) {
    testWidgets(screen.id, skip: skip != null, (tester) async {
      final meta = File('${out.path}/${screen.id}.json');
      void writeMeta({required bool ok}) => meta.writeAsStringSync(
        jsonEncode({'id': screen.id, 'title': screen.title, 'ok': ok}),
      );
      writeMeta(ok: false);

      // スマートフォンの縦の画面（論理 390×844、3 倍）
      tester.view.physicalSize = const Size(1170, 2532);
      tester.view.devicePixelRatio = 3;
      addTearDown(tester.view.reset);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            studentApiProvider.overrideWithValue(_api()),
            clockProvider.overrideWithValue(() => Fixtures.now),
          ],
          child: StudentApp(home: screen.home ?? const StudentFlowScreen()),
        ),
      );
      await tester.pumpAndSettle();
      await screen.navigate(tester);
      await tester.pumpAndSettle();

      await expectLater(
        find.byType(StudentApp),
        matchesGoldenFile('${screen.id}.png'),
      );
      writeMeta(ok: true);
    });
  }
}
