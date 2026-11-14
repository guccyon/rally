import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/session/session_store.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_local_media.dart';
import '../support/fake_student_api.dart';
import '../support/pump.dart';

/// 登録の結果を端末に残し、開き直したら同じ生徒のホームから始める（#1528。段は残さない。ADR 0010）。
/// 開き直しは、同じ [MemorySessionStore] を渡した新しい ProviderContainer で `main` と同じく
/// [StudentFlowNotifier.resumeSavedSession] を呼んでから描くことで再現する。
void main() {
  ProviderContainer containerFor(FakeStudentApi api, SessionStore store) {
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        sessionStoreProvider.overrideWithValue(store),
        localMediaProvider.overrideWithValue(FakeLocalMedia()),
      ],
    );
    addTearDown(container.dispose);
    return container;
  }

  /// `main` と同じ順で開く（残した登録の結果を読んでから最初の画面を描く）
  Future<ProviderContainer> openApp(
    WidgetTester tester,
    FakeStudentApi api,
    SessionStore store,
  ) async {
    final container = containerFor(api, store);
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

  Future<void> register(WidgetTester tester) async {
    expect(find.text('アカウント登録'), findsOneWidget);
    await tester.enterText(
      find.widgetWithText(TextFormField, 'メールアドレス'),
      'yamada@example.com',
    );
    await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
    await tapText(tester, 'テニス');
    await tapText(tester, '登録する');
    expect(find.text('ホーム'), findsOneWidget);
  }

  testWidgets('登録したあと開き直すと、同じ生徒のままホームが開き、ホームの欄はサーバーから読み直す', (tester) async {
    final store = MemorySessionStore();
    await openApp(
      tester,
      FakeStudentApi(myBookings: const [], lessonCompletions: const []),
      store,
    );
    await register(tester);

    final saved = store.read()!;
    expect(saved.accessToken, Fixtures.registered.accessToken);
    expect(saved.student.studentId, Fixtures.studentId);

    // 開き直す
    final api = FakeStudentApi();
    final container = await openApp(tester, api, store);

    expect(find.text('アカウント登録'), findsNothing);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('山田さん、こんにちは'), findsOneWidget);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.home);
    expect(state.accessToken, Fixtures.registered.accessToken);
    expect(state.student!.studentId, Fixtures.studentId);
    expect(api.calls.first, 'getMyStudentProfile');
    expect(
      api.calls,
      containsAll([
        'listMyBookings',
        'getMyEntitlement',
        'listMessageThreads',
        'listMyVideos',
        'listFavoriteCoaches',
        'listMyNotifications',
        'listMyLessonCompletions',
      ]),
    );
    expect(api.calls, isNot(contains('registerAccount')));
  });

  testWidgets('開き直したときは読み直した生徒を残し直す（表示名を変えていればそれが出る）', (tester) async {
    final store = MemorySessionStore()
      ..save(
        SavedSession(
          accessToken: Fixtures.registered.accessToken,
          student: Fixtures.student,
        ),
      );
    final api = FakeStudentApi(
      student: Student(
        studentId: Fixtures.studentId,
        displayName: '山田花子',
        sport: Fixtures.student.sport,
        createdAt: Fixtures.student.createdAt,
      ),
    );
    await openApp(tester, api, store);

    expect(find.text('山田花子さん、こんにちは'), findsOneWidget);
    expect(store.read()!.student.displayName, '山田花子');
  });

  testWidgets('段の状態は残さない: 予約の確定待ちの画面で開き直しても、ホームから始まる', (tester) async {
    final store = MemorySessionStore();
    await openApp(
      tester,
      FakeStudentApi(myBookings: const [], lessonCompletions: const []),
      store,
    );
    await register(tester);
    await tapText(tester, 'スロットを探して予約する');
    await tapText(tester, '体験予約する');
    expect(find.text('予約の確定待ち'), findsOneWidget);

    // 残っているのはトークンと生徒だけ
    final json = jsonDecode(store.raw!) as Map<String, dynamic>;
    expect(json.keys, unorderedEquals(['access_token', 'student']));

    final container = await openApp(tester, FakeStudentApi(), store);
    expect(find.text('予約の確定待ち'), findsNothing);
    expect(find.text('ホーム'), findsOneWidget);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.home);
    expect(state.booking, isNull);
    expect(state.bookedSlot, isNull);
    expect(state.session, isNull);
  });

  testWidgets('トークンが無効（401）なら残したものを消して、理由の文言と登録の画面を出す', (tester) async {
    final store = MemorySessionStore()
      ..save(
        SavedSession(accessToken: 'expired-token', student: Fixtures.student),
      );
    final api = FakeStudentApi(
      failures: const {
        'getMyStudentProfile': StudentApiException(
          statusCode: 401,
          error: ApiError(
            code: ErrorCode.unauthorized,
            message: 'unauthorized',
          ),
        ),
      },
    );
    final container = await openApp(tester, api, store);

    expect(find.text('アカウント登録'), findsOneWidget);
    expect(store.raw, isNull);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.register);
    expect(state.accessToken, isNull);
    expect(state.student, isNull);
    expect(state.busy, isFalse);
    expect(state.errorMessage, 'ログインの有効期限が切れました。登録からやり直してください。');
    expect(find.text('ログインの有効期限が切れました。登録からやり直してください。'), findsOneWidget);
    expect(api.calls, ['getMyStudentProfile']);

    // 登録し直せば、新しいトークンを残す
    await register(tester);
    expect(store.read()!.accessToken, Fixtures.registered.accessToken);
  });

  testWidgets('通信の失敗では残したものを消さず、同じ生徒のままホームを開く', (tester) async {
    final store = MemorySessionStore()
      ..save(
        SavedSession(
          accessToken: Fixtures.registered.accessToken,
          student: Fixtures.student,
        ),
      );
    final api = FakeStudentApi(
      failures: const {'getMyStudentProfile': Fixtures.unreachable},
    );
    await openApp(tester, api, store);

    expect(find.text('山田さん、こんにちは'), findsOneWidget);
    expect(store.read(), isNotNull);
    expect(api.calls, contains('listMyBookings'));
  });

  testWidgets('残していなければ登録の画面のまま、API を呼ばない', (tester) async {
    final api = FakeStudentApi();
    await openApp(tester, api, MemorySessionStore());

    expect(find.text('アカウント登録'), findsOneWidget);
    expect(api.calls, isEmpty);
  });
}
