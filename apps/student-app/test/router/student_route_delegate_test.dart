import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/session/session_store.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_browser_history.dart';
import '../support/fake_local_media.dart';
import '../support/fake_student_api.dart';
import '../support/pump.dart';

/// 段の移動をブラウザーの履歴に写し、「戻る」でアプリから抜けず、アプリの閉じる・戻るのボタンと同じ行き先に移る（#1519）。
/// ホームで「戻る」を押してもアプリから抜けない（#1584）。
/// ブラウザーは [FakeBrowserHistory] で、アプリが報告した項目を積み・置き換え、「戻る」で 1 つ前の項目をアプリに渡す
void main() {
  Future<(FakeBrowserHistory, FakeStudentApi)> openApp(
    WidgetTester tester, {
    FakeStudentApi? api,
    SessionStore? store,
    FakeLocalMedia? media,
  }) async {
    final history = FakeBrowserHistory(tester);
    final fake =
        api ??
        FakeStudentApi(myBookings: const [], lessonCompletions: const []);
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(fake),
        localMediaProvider.overrideWithValue(media ?? FakeLocalMedia()),
        sessionStoreProvider.overrideWithValue(store ?? MemorySessionStore()),
      ],
    );
    addTearDown(container.dispose);
    container.read(studentFlowProvider.notifier).resumeSavedSession();
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const StudentApp(),
      ),
    );
    await tester.pumpAndSettle();
    return (history, fake);
  }

  Future<void> register(WidgetTester tester) async {
    await tester.enterText(
      find.widgetWithText(TextFormField, 'メールアドレス'),
      'yamada@example.com',
    );
    await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
    await tapText(tester, 'テニス');
    await tapText(tester, '登録する');
    expect(find.text('ホーム'), findsOneWidget);
  }

  Future<void> book(WidgetTester tester) async {
    await tapText(tester, 'スロットを探して予約する');
    await tapText(tester, '体験予約する');
    expect(find.text('予約の確定待ち'), findsOneWidget);
  }

  Future<void> toLiveLesson(WidgetTester tester) async {
    await book(tester);
    await tapText(tester, '予約を確定する');
    await tapText(tester, 'ビデオ通話で受ける');
    await tapText(tester, '入室する');
    expect(find.text('コーチと通話中です\n映像が表示されなくても、故障ではありません'), findsOneWidget);
  }

  testWidgets('登録からホームへは履歴を置き換え、スロット検索の「戻る」でホームに戻る。ホームが履歴の底で、登録の画面には戻らない', (
    tester,
  ) async {
    final (history, _) = await openApp(tester);
    expect(find.text('アカウント登録'), findsOneWidget);
    // アプリを開いたページの項目（URL はホーム）の上に積む
    expect(history.entries.map((e) => e.uri), ['/home', '/register']);

    await register(tester);
    expect(history.entries.map((e) => e.uri), ['/home', '/home']);

    await tapText(tester, 'スロットを探して予約する');
    expect(find.text('スロットを探す'), findsOneWidget);
    expect(history.entries.map((e) => e.uri), [
      '/home',
      '/home',
      '/slot-search',
    ]);

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('スロットを探す'), findsNothing);

    // ホームの「戻る」はアプリを開いたページの項目に移るが、ホームのままホームの項目を積み直す（登録の画面は残っていない）
    for (var i = 0; i < 3; i++) {
      await history.back();
      expect(history.leftApp, isFalse);
      expect(find.text('ホーム'), findsOneWidget);
      expect(find.text('アカウント登録'), findsNothing);
      expect(history.current.uri, '/home');
    }
    expect(history.entries.map((e) => e.uri), isNot(contains('/register')));
  });

  testWidgets('プロフィールから開いた月額契約の「戻る」はプロフィールに戻り、続けての「戻る」でホームへ。ホームでは抜けない（#1584）', (
    tester,
  ) async {
    final (history, api) = await openApp(tester);
    await register(tester);
    await tester.tap(find.byTooltip('プロフィール'));
    await tester.pumpAndSettle();
    await tapText(tester, '月額契約へ進む');
    expect(find.text('月額契約を始める'), findsOneWidget);
    expect(history.current.uri, '/subscription');
    api.calls.clear();

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.byTooltip('プロフィールを閉じる'), findsOneWidget);
    expect(find.text('まだ月額契約はありません'), findsOneWidget);
    expect(history.current.uri, '/profile');
    // 月額契約の欄は受講権を読み直して出す
    expect(api.calls, contains('getMyEntitlement'));

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.byTooltip('プロフィールを閉じる'), findsNothing);

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('ホーム'), findsOneWidget);
  });

  testWidgets('プロフィールから月額契約を始めたあとの「戻る」もプロフィールに戻り、月額契約の欄に始めたプランを出す', (
    tester,
  ) async {
    final (history, _) = await openApp(tester);
    await register(tester);
    await tester.tap(find.byTooltip('プロフィール'));
    await tester.pumpAndSettle();
    await tapText(tester, '月額契約へ進む');
    await tapText(tester, '月額契約を始める');
    expect(find.text('月額契約を開始しました'), findsOneWidget);

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.byTooltip('プロフィールを閉じる'), findsOneWidget);
    expect(find.text('月額契約へ進む'), findsNothing);
  });

  testWidgets('ホームから開いた自分の予約を「戻る」で閉じたあとも、ホームの「戻る」でアプリから抜けない', (tester) async {
    final (history, _) = await openApp(tester);
    await register(tester);
    await tester.tap(find.byTooltip('自分の予約'));
    await tester.pumpAndSettle();
    expect(history.current.uri, '/my-bookings');

    await history.back();
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.byTooltip('自分の予約を閉じる'), findsNothing);

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('ホーム'), findsOneWidget);
    expect(history.current.uri, '/home');
  });

  testWidgets('脇の画面の「戻る」は閉じるボタンと同じく開いた段に戻り、もう一度の「戻る」でホームへ', (tester) async {
    final (history, _) = await openApp(tester);
    await register(tester);
    await tapText(tester, 'スロットを探して予約する');
    await tester.tap(find.byTooltip('自分の予約'));
    await tester.pumpAndSettle();
    expect(find.byTooltip('自分の予約を閉じる'), findsOneWidget);
    expect(history.current.uri, '/my-bookings');

    await history.back();
    expect(find.text('スロットを探す'), findsOneWidget);
    expect(history.leftApp, isFalse);
    expect(history.current.uri, '/slot-search');

    await history.back();
    expect(find.text('ホーム'), findsOneWidget);
    expect(history.leftApp, isFalse);
  });

  testWidgets('アプリバーでホームに戻ったあとの「戻る」も、ホームにとどまる', (tester) async {
    final (history, _) = await openApp(tester);
    await register(tester);
    await tapText(tester, 'スロットを探して予約する');
    await tester.tap(find.byTooltip('ホーム'));
    await tester.pumpAndSettle();

    for (var i = 0; i < 3; i++) {
      await history.back();
      expect(history.leftApp, isFalse);
      expect(find.text('ホーム'), findsOneWidget);
    }
  });

  testWidgets('確定待ちの画面の「戻る」はホームへ戻り、確定待ちの予約は「次にやること」に残る', (tester) async {
    final api = FakeStudentApi(
      myBookings: const [],
      lessonCompletions: const [],
    );
    final (history, _) = await openApp(tester, api: api);
    await register(tester);
    await book(tester);
    api.nextActions
      ..clear()
      ..add(Fixtures.nextConfirmBooking);

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('体験予約がまだ確定していません'), findsOneWidget);
  });

  testWidgets('入室前チェックの「戻る」はホームへ戻り、カメラ・マイクを止める', (tester) async {
    final media = FakeLocalMedia();
    final (history, _) = await openApp(tester, media: media);
    await register(tester);
    await book(tester);
    await tapText(tester, '予約を確定する');
    await tapText(tester, 'ビデオ通話で受ける');
    expect(find.text('入室前チェック'), findsOneWidget);
    media.calls.clear();

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('入室前チェック'), findsNothing);
    expect(media.calls, contains('stopAll'));
  });

  testWidgets('通話中の「戻る」は通話の画面にとどまって確認を出し、「通話を続ける」でそのまま通話を続ける', (tester) async {
    final (history, api) = await openApp(tester);
    await register(tester);
    await toLiveLesson(tester);

    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('通話を終了しますか？'), findsOneWidget);
    expect(find.text('コーチと通話中です\n映像が表示されなくても、故障ではありません'), findsOneWidget);

    await tapText(tester, '通話を続ける');
    expect(find.text('通話を終了しますか？'), findsNothing);
    expect(find.text('コーチと通話中です\n映像が表示されなくても、故障ではありません'), findsOneWidget);
    expect(api.calls, isNot(contains('endLiveLessonSession')));

    // 確認を出したまま「戻る」を押すと、確認を閉じるだけ（通話は続く）
    await history.back();
    expect(find.text('通話を終了しますか？'), findsOneWidget);
    await history.back();
    expect(history.leftApp, isFalse);
    expect(find.text('通話を終了しますか？'), findsNothing);
    expect(find.text('コーチと通話中です\n映像が表示されなくても、故障ではありません'), findsOneWidget);
    expect(api.calls, isNot(contains('endLiveLessonSession')));
  });

  testWidgets('通話中の「戻る」の確認で「通話を終了する」を押すと、通話を終えて完了の記録へ進む', (tester) async {
    final (history, api) = await openApp(tester);
    await register(tester);
    await toLiveLesson(tester);

    await history.back();
    // 画面の「通話を終了する」ではなく、確認の「通話を終了する」を押す
    await tester.tap(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.text('通話を終了する'),
      ),
    );
    await tester.pumpAndSettle();
    expect(api.calls.last, 'endLiveLessonSession');
    expect(find.text('レッスンの完了'), findsOneWidget);
    expect(history.leftApp, isFalse);
  });

  testWidgets('再読み込み・URL を開いたときは URL の段に関わらずホームを開き、「進む」でも段を復元しない', (
    tester,
  ) async {
    final store = MemorySessionStore();
    store.save(
      SavedSession(
        accessToken: Fixtures.registered.accessToken,
        student: Fixtures.registered.student,
      ),
    );
    tester.binding.platformDispatcher.defaultRouteNameTestValue =
        '/slot-search';
    addTearDown(
      tester.binding.platformDispatcher.clearDefaultRouteNameTestValue,
    );

    final (history, _) = await openApp(tester, store: store);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('スロットを探す'), findsNothing);
    expect(history.current.uri, '/home');

    // ホームから進んで戻ったあとの「進む」
    await tapText(tester, 'スロットを探して予約する');
    await history.back();
    expect(find.text('ホーム'), findsOneWidget);
    await history.forward();
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('スロットを探す'), findsNothing);
    expect(history.current.uri, '/home');

    // 開き直したホームの「戻る」でもアプリから抜けない
    for (var i = 0; i < 3; i++) {
      await history.back();
      expect(history.leftApp, isFalse);
      expect(find.text('ホーム'), findsOneWidget);
    }
  });

  testWidgets('端末の「戻る」（Android の戻るボタンなど）も同じ行き先に移し、ホームでだけアプリを抜ける', (
    tester,
  ) async {
    await openApp(tester);
    await register(tester);
    await tapText(tester, 'スロットを探して予約する');

    expect(await tester.binding.handlePopRoute(), isTrue);
    await tester.pumpAndSettle();
    expect(find.text('ホーム'), findsOneWidget);

    // ホームでは抜ける（アプリは false を返して端末に任せる）
    final router = tester.widget<Router<Object>>(find.byType(Router<Object>));
    expect(await router.routerDelegate.popRoute(), isFalse);
  });
}
