import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/session/session_store.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';
import 'package:rally_student_app/ui/components/rally_button.dart';
import 'package:rally_student_app/ui/labels.dart';

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
    SessionStore store, {
    DateTime? now,
  }) async {
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        sessionStoreProvider.overrideWithValue(store),
        localMediaProvider.overrideWithValue(FakeLocalMedia()),
        if (now != null) clockProvider.overrideWithValue(() => now),
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

  group('予約の応答を待つあいだに画面が描き直されても（#1730）', () {
    /// 「ログアウトする」の行を [kind]（マウス・タッチ）で押し、`listMyBookings` の応答を待つあいだに
    /// 呼んでいる間の表示（進み具合の帯）を描いてから応答を返す（staging のように応答がフレームをまたぐ）
    Future<void> tapLogoutAndAnswerLater(
      WidgetTester tester,
      FakeStudentApi api,
      PointerDeviceKind kind,
    ) async {
      final gate = Completer<void>();
      api.myBookingsGate = gate;
      final row = find.text('ログアウトする');
      await tester.ensureVisible(row);
      await tester.tap(row, kind: kind);
      await tester.pump();
      await tester.pump();
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      gate.complete();
      await tester.pumpAndSettle();
    }

    for (final kind in [PointerDeviceKind.mouse, PointerDeviceKind.touch]) {
      testWidgets('${kind.name}: 確かめが出て、「ログアウトする」で登録の画面になり、開き直しても前の生徒は出ない', (
        tester,
      ) async {
        final store = savedStore();
        final api = FakeStudentApi();
        final container = await openApp(tester, api, store);
        await openProfile(tester);

        await tapLogoutAndAnswerLater(tester, api, kind);

        expect(find.text('ログアウトしますか？'), findsOneWidget);
        await tester.tap(confirmButton, kind: kind);
        await tester.pumpAndSettle();

        expect(find.text('アカウント登録'), findsOneWidget);
        expect(find.text('山田さん、こんにちは'), findsNothing);
        expect(container.read(studentFlowProvider).accessToken, isNull);
        expect(store.raw, isNull);

        // 別の人が同じブラウザーで開き直す
        final next = FakeStudentApi();
        final reopened = await openApp(tester, next, store);
        expect(find.text('アカウント登録'), findsOneWidget);
        expect(find.text('山田さん、こんにちは'), findsNothing);
        expect(reopened.read(studentFlowProvider).accessToken, isNull);
        expect(next.calls, isEmpty);
      });
    }

    testWidgets('「やめる」を押せばプロフィールのまま', (tester) async {
      final store = savedStore();
      final api = FakeStudentApi();
      final container = await openApp(tester, api, store);
      await openProfile(tester);

      await tapLogoutAndAnswerLater(tester, api, PointerDeviceKind.touch);
      await tapText(tester, 'やめる');

      expect(find.text('ログアウトしますか？'), findsNothing);
      expect(container.read(studentFlowProvider).step, FlowStep.profile);
      expect(store.read(), isNotNull);
    });

    testWidgets('確定待ちの予約があれば確かめに日時を書き、キャンセルしてから登録の画面にする', (tester) async {
      final store = savedStore();
      final api = FakeStudentApi();
      await openApp(tester, api, store, now: Fixtures.now);
      await openProfile(tester);

      await tapLogoutAndAnswerLater(tester, api, PointerDeviceKind.touch);

      expect(find.text('ログアウトしますか？'), findsOneWidget);
      expect(find.textContaining('確定待ちの予約（'), findsOneWidget);
      await tester.tap(confirmButton);
      await tester.pumpAndSettle();

      expect(api.calls.last, 'cancelBooking');
      expect(find.text('アカウント登録'), findsOneWidget);
      expect(store.raw, isNull);
    });
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

  group('ほかのタブでログアウトしたとき（#1669）', () {
    /// 次の人が別のタブで登録した結果（別のトークンと生徒）
    final nextPerson = SavedSession(
      accessToken: 'mock.student.next',
      student: Student(
        studentId: '77777777-7777-4777-8777-777777777777',
        displayName: '中学生ゆうき',
        sport: Sport.tennis,
        createdAt: Fixtures.now,
      ),
    );

    /// このタブでチャットのスレッド（担当コーチとの相談）を開いておく
    Future<ProviderContainer> openThread(
      WidgetTester tester,
      FakeStudentApi api,
      MemorySessionStore store,
    ) async {
      final container = await openApp(tester, api, store);
      await container.read(chatProvider.notifier).openThread(Fixtures.thread);
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.messages);
      expect(container.read(chatProvider).messages, isNotEmpty);
      return container;
    }

    void expectLoggedOut(ProviderContainer container) {
      expect(find.text('アカウント登録'), findsOneWidget);
      expect(find.text('山田さん、こんにちは'), findsNothing);
      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.register);
      expect(state.accessToken, isNull);
      expect(state.student, isNull);
      expect(state.myBookings, isNull);
      expect(container.read(chatProvider).thread, isNull);
      expect(container.read(chatProvider).threads, isNull);
      expect(container.read(chatProvider).messages, isNull);
      expect(container.read(subscriptionProvider).entitlement, isNull);
    }

    testWidgets('開いていたタブもチャットとホームの状態を捨てて登録の画面になり、サーバーには何も送らない', (tester) async {
      final store = savedStore();
      final api = FakeStudentApi();
      final container = await openThread(tester, api, store);
      final callsBefore = List.of(api.calls);

      store.replaceElsewhere(null);
      await tester.pumpAndSettle();

      expectLoggedOut(container);
      expect(api.calls, callsBefore);
    });

    /// このタブが次の人のホームになっている（前の生徒のトークン・スレッドは捨て、次の人の登録の結果は消さない。#1805）
    void expectFollowedNextPerson(
      ProviderContainer container,
      MemorySessionStore store,
    ) {
      expect(find.text('中学生ゆうきさん、こんにちは'), findsOneWidget);
      expect(find.text('山田さん、こんにちは'), findsNothing);
      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.home);
      expect(state.accessToken, nextPerson.accessToken);
      expect(state.student!.studentId, nextPerson.student.studentId);
      expect(container.read(chatProvider).thread, isNull);
      expect(container.read(chatProvider).messages, isNull);
      expect(store.read()!.accessToken, nextPerson.accessToken);
    }

    testWidgets('次の人が別のタブで登録したら、このタブに前の生徒は出ず次の人のホームになり、次の人の登録の結果は消さない', (
      tester,
    ) async {
      final store = savedStore();
      final api = FakeStudentApi();
      final container = await openThread(tester, api, store);

      store.replaceElsewhere(null);
      await tester.pumpAndSettle();
      expectLoggedOut(container);
      api.student = nextPerson.student;
      store.replaceElsewhere(nextPerson);
      await tester.pumpAndSettle();

      expectFollowedNextPerson(container, store);
      expect(api.calls, isNot(contains('registerAccount')));
    });

    testWidgets('ログアウトの知らせを受けずに次の人の登録だけが届いても、前の生徒のトークンを捨てて前の生徒として送れない', (
      tester,
    ) async {
      final store = savedStore();
      final api = FakeStudentApi();
      final container = await openThread(tester, api, store);

      api.student = nextPerson.student;
      store.replaceElsewhere(nextPerson);
      await tester.pumpAndSettle();

      expectFollowedNextPerson(container, store);
      // 前の生徒のスレッドのメッセージ欄はもうなく、前の生徒として送らない
      expect(find.text('送信する'), findsNothing);
      expect(api.requests.whereType<SendMessageRequest>(), isEmpty);
    });

    testWidgets('登録の画面のままのタブも、別のタブで次の人が登録したら次の人のホームになる（#1805）', (tester) async {
      final store = MemorySessionStore();
      final api = FakeStudentApi()..student = nextPerson.student;
      final container = await openApp(tester, api, store);
      expect(find.text('アカウント登録'), findsOneWidget);

      store.replaceElsewhere(nextPerson);
      await tester.pumpAndSettle();

      expectFollowedNextPerson(container, store);
      expect(api.calls, isNot(contains('registerAccount')));
    });

    testWidgets('登録の画面のままのタブで登録を押しても、別のタブで先に登録した人がいれば登録せず、その人のホームにする（#1805）', (
      tester,
    ) async {
      final store = MemorySessionStore();
      final api = FakeStudentApi()..student = nextPerson.student;
      final container = await openApp(tester, api, store);
      // 知らせが届く前に、ほかのタブが登録の結果を残した
      store.save(nextPerson);

      await container
          .read(studentFlowProvider.notifier)
          .register(
            email: 'k@example.com',
            displayName: '夜QA k',
            sport: Sport.tennis,
          );
      await tester.pumpAndSettle();

      expectFollowedNextPerson(container, store);
      expect(api.calls, isNot(contains('registerAccount')));
    });

    testWidgets('次の人のホームに切り替わったあとに前の生徒のトークンが 401 でも、次の人の登録の結果は消さない', (
      tester,
    ) async {
      final store = MemorySessionStore();
      final api = FakeStudentApi()..student = nextPerson.student;
      final container = await openApp(tester, api, store);
      final gate = Completer<StudentApiException?>();
      api.nextProfileGate = gate;
      store.replaceElsewhere(
        SavedSession(
          accessToken: Fixtures.registered.accessToken,
          student: Fixtures.student,
        ),
      );
      await tester.pump();
      // 前の生徒のトークンを確かめているあいだに、次の人が別のタブで登録した
      store.replaceElsewhere(nextPerson);
      await tester.pumpAndSettle();
      gate.complete(
        const StudentApiException(
          statusCode: 401,
          error: ApiError(
            code: ErrorCode.unauthorized,
            message: 'unauthorized',
          ),
        ),
      );
      await tester.pumpAndSettle();

      expectFollowedNextPerson(container, store);
    });

    testWidgets('ほかのタブが同じ生徒のまま残し直しただけ（表示名を変えたなど）なら、このタブはそのまま', (tester) async {
      final store = savedStore();
      final container = await openApp(tester, FakeStudentApi(), store);

      store.replaceElsewhere(
        SavedSession(
          accessToken: Fixtures.registered.accessToken,
          student: Fixtures.student,
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('山田さん、こんにちは'), findsOneWidget);
      expect(
        container.read(studentFlowProvider).accessToken,
        Fixtures.registered.accessToken,
      );
    });
  });

  group('確定待ちの予約があるとき（#1718）', () {
    /// [Fixtures.now] の時計で開く。[Fixtures.myBookings] のうち確定待ちでまだ始まっていないのは 12 時間後の `booking-soon` だけ
    Future<ProviderContainer> openAtFixtureNow(
      WidgetTester tester,
      FakeStudentApi api,
      SessionStore store,
    ) => openApp(tester, api, store, now: Fixtures.now);

    MyBooking soon(FakeStudentApi api) => api.myBookings.singleWhere(
      (b) => b.booking.bookingId == 'booking-soon',
    );

    testWidgets('確かめに確定待ちの予約をキャンセルすることを書き、「ログアウトする」でキャンセルしてから登録の画面にする', (
      tester,
    ) async {
      final store = savedStore();
      final api = FakeStudentApi();
      final container = await openAtFixtureNow(tester, api, store);
      final slot = soon(api).slot;

      await openProfile(tester);
      await tapText(tester, 'ログアウトする');

      // 確定待ちの予約の有無はサーバーから読む（ADR 0010）
      expect(api.calls.last, 'listMyBookings');
      expect(find.text('ログアウトしますか？'), findsOneWidget);
      expect(
        find.text(
          '確定待ちの予約（${formatTimeRange(slot.startsAt, slot.endsAt)}）があります。'
          'ログアウトするとこの予約を確定できなくなるため、キャンセルしてからログアウトします。'
          '$cancelConsumesQuotaNotice'
          '山田さんの予約・チャット・月額契約をこの端末に出さないようにし、登録の画面に戻ります。'
          'ログインの画面はまだないため、ログアウトするとこのアカウントには戻れません。',
        ),
        findsOneWidget,
      );
      final callsBefore = api.calls.length;

      await tester.tap(confirmButton);
      await tester.pumpAndSettle();

      expect(api.calls.sublist(callsBefore), ['cancelBooking']);
      expect(soon(api).booking.status, BookingStatus.cancelled);
      // 確定待ちでない予約（確定済み・キャンセル済み・終わったもの）には触らない
      expect(
        api.myBookings
            .where((b) => b.booking.bookingId != 'booking-soon')
            .map((b) => b.booking.status),
        [
          BookingStatus.confirmed,
          BookingStatus.cancelled,
          BookingStatus.confirmed,
        ],
      );
      expect(find.text('アカウント登録'), findsOneWidget);
      expect(store.raw, isNull);
      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.register);
      expect(state.accessToken, isNull);
      expect(state.myBookings, isNull);
    });

    testWidgets('確定待ちの予約が 2 件あれば、開始のいちばん早い予約の日時と残りの件数を書き、どちらもキャンセルする', (
      tester,
    ) async {
      final store = savedStore();
      final api = FakeStudentApi(
        myBookings: [
          Fixtures.myBooking(
            bookingId: 'booking-pending-later',
            startsIn: const Duration(days: 3),
            status: BookingStatus.pending,
          ),
          Fixtures.myBooking(
            bookingId: 'booking-pending-soon',
            startsIn: const Duration(days: 2),
            status: BookingStatus.pending,
          ),
        ],
      );
      await openAtFixtureNow(tester, api, store);
      final slot = api.myBookings.last.slot;

      await openProfile(tester);
      await tapText(tester, 'ログアウトする');

      expect(
        find.textContaining(
          '確定待ちの予約（${formatTimeRange(slot.startsAt, slot.endsAt)} ほか 1 件）があります。',
        ),
        findsOneWidget,
      );
      // どちらも枠を使わずにキャンセルできる（24 時間より前）ので、枠の案内は出さない
      expect(find.textContaining(cancelConsumesQuotaNotice), findsNothing);

      await tester.tap(confirmButton);
      await tester.pumpAndSettle();

      expect(
        api.myBookings.map((b) => b.booking.status),
        everyElement(BookingStatus.cancelled),
      );
      expect(find.text('アカウント登録'), findsOneWidget);
    });

    testWidgets('キャンセルできなければログアウトせず、プロフィールのまま失敗を知らせる', (tester) async {
      final store = savedStore();
      final api = FakeStudentApi(
        failures: {'cancelBooking': Fixtures.unreachable},
      );
      final container = await openAtFixtureNow(tester, api, store);

      await openProfile(tester);
      await tapText(tester, 'ログアウトする');
      await tester.tap(confirmButton);
      await tester.pumpAndSettle();

      expect(
        find.text(
          '確定待ちの予約をキャンセルできなかったため、ログアウトしていません。'
          'サーバーにつながりませんでした。通信状態を確かめてください。',
        ),
        findsOneWidget,
      );
      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.profile);
      expect(state.accessToken, Fixtures.registered.accessToken);
      expect(store.read(), isNotNull);
      expect(soon(api).booking.status, BookingStatus.pending);
    });

    testWidgets('「やめる」を押せば、キャンセルもログアウトもしない', (tester) async {
      final store = savedStore();
      final api = FakeStudentApi();
      final container = await openAtFixtureNow(tester, api, store);

      await openProfile(tester);
      await tapText(tester, 'ログアウトする');
      await tapText(tester, 'やめる');

      expect(api.calls, isNot(contains('cancelBooking')));
      expect(soon(api).booking.status, BookingStatus.pending);
      expect(container.read(studentFlowProvider).step, FlowStep.profile);
      expect(store.read(), isNotNull);
    });

    testWidgets('予約を読めなければ確かめを出さず、ログアウトせずに失敗を知らせる', (tester) async {
      final store = savedStore();
      final api = FakeStudentApi(
        failures: {'listMyBookings': Fixtures.unreachable},
      );
      final container = await openAtFixtureNow(tester, api, store);

      await openProfile(tester);
      await tapText(tester, 'ログアウトする');

      expect(find.text('ログアウトしますか？'), findsNothing);
      expect(
        find.text(
          '確定待ちの予約があるかを確かめられなかったため、ログアウトしていません。'
          'サーバーにつながりませんでした。通信状態を確かめてください。',
        ),
        findsOneWidget,
      );
      expect(container.read(studentFlowProvider).step, FlowStep.profile);
      expect(store.read(), isNotNull);
    });
  });
}
