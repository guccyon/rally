import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/session/session_store.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';
import 'package:rally_student_app/ui/components/rally_button.dart';

import '../support/fake_local_media.dart';
import '../support/fake_student_api.dart';
import '../support/pump.dart';

/// プロフィールの「ログアウトする」で、端末に残した登録の結果と前の生徒の状態を捨てて登録の画面にする（#1588）。
/// 開き直しは saved_session_test.dart と同じく、同じ [MemorySessionStore] を渡した新しい ProviderContainer で
/// `main` と同じ順（[StudentFlowNotifier.resumeSavedSession] を呼んでから描く）で再現する。
void main() {
  /// `main` と同じ順で開く（残した登録の結果を読んでから最初の画面を描く）
  Future<ProviderContainer> openApp(
    WidgetTester tester,
    FakeStudentApi api,
    SessionStore store,
  ) async {
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        sessionStoreProvider.overrideWithValue(store),
        localMediaProvider.overrideWithValue(FakeLocalMedia()),
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
    return container;
  }

  MemorySessionStore savedStore() {
    final store = MemorySessionStore()
      ..save(
        SavedSession(
          accessToken: Fixtures.registered.accessToken,
          student: Fixtures.student,
        ),
      );
    return store;
  }

  Future<void> openProfile(WidgetTester tester) async {
    await tester.tap(find.byTooltip('プロフィール'));
    await tester.pumpAndSettle();
  }

  /// 確かめのダイアログの「ログアウトする」
  final confirmButton = find.widgetWithText(RallyButton, 'ログアウトする');

  testWidgets('プロフィールの「ログアウトする」は確かめてから、残した登録の結果と前の生徒の状態を捨てて登録の画面にする', (
    tester,
  ) async {
    final store = savedStore();
    final api = FakeStudentApi();
    final container = await openApp(tester, api, store);
    expect(find.text('山田さん、こんにちは'), findsOneWidget);
    // ホームの欄（チャットのスレッド・受講権）を読んである
    expect(container.read(chatProvider).threads, isNotNull);

    await openProfile(tester);
    await tapText(tester, 'ログアウトする');

    expect(find.text('ログアウトしますか？'), findsOneWidget);
    expect(
      find.text(
        '山田さんの予約・チャット・月額契約をこの端末に出さないようにし、登録の画面に戻ります。'
        'ログインの画面はまだないため、ログアウトするとこのアカウントには戻れません。',
      ),
      findsOneWidget,
    );
    final callsBefore = List.of(api.calls);

    await tester.tap(confirmButton);
    await tester.pumpAndSettle();

    expect(find.text('アカウント登録'), findsOneWidget);
    expect(find.text('山田さん、こんにちは'), findsNothing);
    expect(store.raw, isNull);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.register);
    expect(state.accessToken, isNull);
    expect(state.student, isNull);
    expect(state.myBookings, isNull);
    expect(container.read(chatProvider).threads, isNull);
    expect(container.read(chatProvider).messages, isNull);
    expect(container.read(subscriptionProvider).entitlement, isNull);
    // サーバーには何も送らない
    expect(api.calls, callsBefore);
  });

  testWidgets('確かめのダイアログで「やめる」を押せば、ログアウトせずプロフィールのまま', (tester) async {
    final store = savedStore();
    final container = await openApp(tester, FakeStudentApi(), store);

    await openProfile(tester);
    await tapText(tester, 'ログアウトする');
    await tapText(tester, 'やめる');

    expect(find.text('ログアウトしますか？'), findsNothing);
    expect(container.read(studentFlowProvider).step, FlowStep.profile);
    expect(
      container.read(studentFlowProvider).accessToken,
      Fixtures.registered.accessToken,
    );
    expect(store.read(), isNotNull);
  });

  testWidgets('ログアウトしたあと開き直しても、前の生徒のホームは出ず登録の画面で、API を呼ばない', (tester) async {
    final store = savedStore();
    await openApp(tester, FakeStudentApi(), store);
    await openProfile(tester);
    await tapText(tester, 'ログアウトする');
    await tester.tap(confirmButton);
    await tester.pumpAndSettle();

    // 別の人が同じブラウザーで開き直す
    final api = FakeStudentApi();
    final container = await openApp(tester, api, store);

    expect(find.text('アカウント登録'), findsOneWidget);
    expect(find.text('山田さん、こんにちは'), findsNothing);
    expect(container.read(studentFlowProvider).accessToken, isNull);
    expect(api.calls, isEmpty);
  });

  testWidgets('ログアウトしたあと、別の表示名で新しく登録して使い始められ、その生徒の登録の結果を残す', (tester) async {
    final store = savedStore();
    final api = FakeStudentApi();
    await openApp(tester, api, store);
    await openProfile(tester);
    await tapText(tester, 'ログアウトする');
    await tester.tap(confirmButton);
    await tester.pumpAndSettle();
    final loggedOutAt = api.calls.length;

    await tester.enterText(
      find.widgetWithText(TextFormField, 'メールアドレス'),
      'yuki@example.com',
    );
    await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '中学生ゆうき');
    await tapText(tester, 'テニス');
    await tapText(tester, '登録する');

    expect(find.text('ホーム'), findsOneWidget);
    final request = api.requests.whereType<RegisterAccountRequest>().single;
    expect(request.displayName, '中学生ゆうき');
    expect(request.email, 'yuki@example.com');
    expect(store.read()!.accessToken, Fixtures.registered.accessToken);
    // ホームの欄は新しい生徒のトークンでサーバーから読み直す（ADR 0010）
    final callsAfter = api.calls.sublist(loggedOutAt);
    expect(callsAfter.first, 'registerAccount');
    expect(callsAfter, containsAll(['listMessageThreads', 'getMyEntitlement']));
  });
}
