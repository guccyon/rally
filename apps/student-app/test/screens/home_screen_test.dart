import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
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

  /// 月額契約（standard）で今月のビデオ通話を 1 回使った受講権
  final entitlement = Entitlement(
    entitlementId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
    studentId: Fixtures.studentId,
    contractType: ContractType.subscription,
    sport: 'tennis',
    plan: SubscriptionPlan.standard,
    unlimitedVideoAndChat: true,
    liveLessonQuota: const LiveLessonQuota(
      month: '2026-10',
      limit: 2,
      used: 1,
      remaining: 1,
    ),
  );

  MessageThread thread(
    String id, {
    required int unreadCount,
    Message? lastMessage,
    String coachId = Fixtures.coachId,
  }) => MessageThread(
    threadId: id,
    studentId: Fixtures.studentId,
    coachId: coachId,
    openedAt: Fixtures.now,
    unreadCount: unreadCount,
    lastMessage: lastMessage,
  );

  Message message(
    String threadId, {
    required MessageKind kind,
    required MessageSenderRole role,
    required String body,
    required int minutes,
  }) => Message(
    messageId: 'm-$threadId',
    threadId: threadId,
    senderId: role == MessageSenderRole.student
        ? Fixtures.studentId
        : Fixtures.coachId,
    senderRole: role,
    kind: kind,
    body: body,
    videoId: kind == MessageKind.text ? null : Fixtures.videoId,
    sentAt: Fixtures.now.add(Duration(minutes: minutes)),
  );

  /// ホームを描いて欄を読む（登録の直後と同じ）
  Future<ProviderContainer> pumpHome(
    WidgetTester tester,
    FakeStudentApi api,
  ) async {
    // 欄をすべて描くよう、縦に長い画面にする（ListView は画面の外の子を作らない）
    tester.view.physicalSize = const Size(800, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
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

  testWidgets('これからの予約を近い順に 3 件まで・今月のビデオ通話の残り・未読のあるスレッドを並べる', (tester) async {
    final soonest = Fixtures.myBooking(
      bookingId: 'booking-1h',
      startsIn: const Duration(hours: 1),
      coachDisplayName: '佐藤',
    );
    final fourth = Fixtures.myBooking(
      bookingId: 'booking-10d',
      startsIn: const Duration(days: 10),
    );
    final api = FakeStudentApi(
      // 新しい順（契約の並び）。キャンセル済みと終わった予約は出さない
      myBookings: [fourth, ...Fixtures.myBookings, soonest],
      entitlement: entitlement,
    );
    final container = await pumpHome(tester, api);

    expect(api.calls, [
      'listMyBookings',
      'getMyEntitlement',
      'listMessageThreads',
    ]);
    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('山田さん、こんにちは'), findsOneWidget);

    final upcoming = container
        .read(studentFlowProvider.notifier)
        .upcomingBookings();
    expect(upcoming.map((b) => b.booking.bookingId), [
      'booking-1h',
      'booking-soon',
      'booking-later',
    ]);
    for (final item in upcoming) {
      expect(
        find.text(formatTimeRange(item.slot.startsAt, item.slot.endsAt)),
        findsOneWidget,
      );
    }
    expect(find.text('テニス・コーチ: 佐藤・確定済み'), findsNWidgets(2));
    expect(find.text('サッカー・コーチ: 鈴木・確定待ち'), findsOneWidget);
    // 4 件目・キャンセル済み・終わった予約は出さない
    for (final item in [
      fourth,
      Fixtures.myBookings[2],
      Fixtures.myBookings[3],
    ]) {
      expect(
        find.text(formatTimeRange(item.slot.startsAt, item.slot.endsAt)),
        findsNothing,
      );
    }

    expect(find.text('月 2 回のうち残り 1 回'), findsOneWidget);
    expect(find.text('プラン: スタンダード'), findsOneWidget);

    // スレッドにはコーチの名前がないので、自分の予約に載っている名前で出す
    expect(find.text('佐藤 コーチ'), findsOneWidget);
    expect(find.text('コーチ: 体験レッスンでお待ちしています'), findsOneWidget);
    expect(find.text('未読 1'), findsOneWidget);
  });

  testWidgets('コーチとのやり取りは、未読のあるスレッドと最後が添削の依頼・返却のスレッドを新しい順に出す', (tester) async {
    final api = FakeStudentApi(
      myBookings: const [],
      threads: [
        thread(
          'read-text',
          unreadCount: 0,
          lastMessage: message(
            'read-text',
            kind: MessageKind.text,
            role: MessageSenderRole.coach,
            body: '読んだメッセージ',
            minutes: 30,
          ),
        ),
        thread(
          'requested',
          unreadCount: 0,
          coachId: 'coach-b',
          lastMessage: message(
            'requested',
            kind: MessageKind.videoReviewRequest,
            role: MessageSenderRole.student,
            body: '動画の添削を依頼しました',
            minutes: 10,
          ),
        ),
        thread(
          'returned',
          unreadCount: 1,
          coachId: 'coach-c',
          lastMessage: message(
            'returned',
            kind: MessageKind.videoReviewReturned,
            role: MessageSenderRole.coach,
            body: '動画の添削を返しました',
            minutes: 20,
          ),
        ),
      ],
    );
    final container = await pumpHome(tester, api);

    final threads = container
        .read(studentFlowProvider.notifier)
        .recentThreads();
    expect(threads.map((t) => t.threadId), ['returned', 'requested']);
    expect(find.text('担当コーチ'), findsNWidgets(2));
    expect(find.textContaining('コーチから添削が返ってきました（'), findsOneWidget);
    expect(find.textContaining('添削を依頼しました（'), findsOneWidget);
    expect(find.text('コーチ: 読んだメッセージ'), findsNothing);
  });

  testWidgets('予約・月額契約・やり取りがなければ、それぞれ「まだ〜はありません」で、添削の依頼は押せない', (tester) async {
    final api = FakeStudentApi(myBookings: const [], threads: const []);
    await pumpHome(tester, api);

    expect(find.text('まだこれからの予約はありません。'), findsOneWidget);
    expect(
      find.text('まだ月額契約はありません。月額契約を始めると、毎月のビデオ通話の残りの回数がここに出ます。'),
      findsOneWidget,
    );
    expect(find.text('まだ新しいやり取りはありません。'), findsOneWidget);
    expect(
      find.text('予約が確定すると担当コーチとのチャットが開き、動画の添削を依頼できるようになります。'),
      findsOneWidget,
    );
    final request = tester.widget<ButtonStyleButton>(
      find.ancestor(
        of: find.text('添削を依頼する'),
        matching: find.bySubtype<ButtonStyleButton>(),
      ),
    );
    expect(request.onPressed, isNull);
    // 受講権の 404 は失敗ではない（失敗の文言を出さない）
    expect(find.textContaining('読み込めませんでした'), findsNothing);
  });

  final failureCases = {
    'listMyBookings': '予約を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。',
    'getMyEntitlement': 'ビデオ通話の残りの回数を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。',
    'listMessageThreads': 'コーチとのやり取りを読み込めませんでした。「ホームを更新する」でもう一度読み込めます。',
  };
  for (final MapEntry(key: operationId, value: text) in failureCases.entries) {
    testWidgets('$operationId が失敗しても他の欄は出し、その欄にだけ読めなかった旨を出す', (tester) async {
      final api = FakeStudentApi(
        entitlement: entitlement,
        failures: {operationId: Fixtures.unreachable},
      );
      await pumpHome(tester, api);

      expect(find.text(text), findsOneWidget);
      expect(find.textContaining('読み込めませんでした'), findsOneWidget);
      // 画面全体の失敗の文言は出さない
      expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsNothing);
      expect(
        find.text('サッカー・コーチ: 鈴木・確定待ち'),
        operationId == 'listMyBookings' ? findsNothing : findsOneWidget,
      );
      expect(
        find.text('月 2 回のうち残り 1 回'),
        operationId == 'getMyEntitlement' ? findsNothing : findsOneWidget,
      );
      expect(
        find.text('未読 1'),
        operationId == 'listMessageThreads' ? findsNothing : findsOneWidget,
      );
    });
  }

  testWidgets('読み直して読めたら、読めなかった旨を消す', (tester) async {
    final api = FakeStudentApi(
      entitlement: entitlement,
      failures: {'getMyEntitlement': Fixtures.unreachable},
    );
    final container = await pumpHome(tester, api);
    expect(find.textContaining('読み込めませんでした'), findsOneWidget);

    api.failures.clear();
    await tapText(tester, 'ホームを更新する');

    expect(find.textContaining('読み込めませんでした'), findsNothing);
    expect(find.text('月 2 回のうち残り 1 回'), findsOneWidget);
    expect(container.read(studentFlowProvider).homeFailures, isEmpty);
  });

  testWidgets('「予約をすべて見る」で自分の予約を開き、閉じるとホームに戻る', (tester) async {
    final api = FakeStudentApi(entitlement: entitlement);
    final container = await pumpHome(tester, api);

    await tapText(tester, '予約をすべて見る');
    expect(container.read(studentFlowProvider).step, FlowStep.myBookings);
    expect(find.text('自分の予約'), findsOneWidget);

    await tester.tap(find.byTooltip('自分の予約を閉じる'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.home);
  });

  testWidgets('「スロットを探して予約する」で生徒のスポーツのスロットを探す', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpHome(tester, api);
    api.calls.clear();

    await tapText(tester, 'スロットを探して予約する');

    expect(container.read(studentFlowProvider).step, FlowStep.slotSearch);
    expect(api.calls.first, 'searchSlots');
    expect(api.requests.whereType<Sport>().last, Sport.tennis);
  });

  testWidgets('やり取りのスレッドを押すとそのチャットを開き（既読にする）、チャットを閉じるとホームに戻る', (tester) async {
    final api = FakeStudentApi(entitlement: entitlement);
    final container = await pumpHome(tester, api);
    api.calls.clear();

    await tapText(tester, '佐藤 コーチ');

    expect(find.text('担当コーチとのチャット'), findsOneWidget);
    expect(api.calls, ['markMessageThreadRead', 'listMessages']);
    expect(container.read(studentFlowProvider).chatReturnStep, FlowStep.home);

    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('チャットを閉じる'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.home);
  });

  testWidgets('「添削を依頼する」でスレッドを開いてすぐに添削の依頼を出し、依頼すると submitVideo を送る', (
    tester,
  ) async {
    final api = FakeStudentApi(entitlement: entitlement);
    final container = await pumpHome(tester, api);
    api.calls.clear();

    await tapText(tester, '添削を依頼する');

    expect(find.text('担当コーチとのチャット'), findsOneWidget);
    expect(find.text('動画の添削を依頼する'), findsOneWidget);
    expect(
      container.read(studentFlowProvider).promptVideoReviewRequest,
      isFalse,
    );
    await tapText(tester, '動画を選ぶ（モック）');
    await tapText(tester, '依頼する');

    expect(api.calls, [
      'markMessageThreadRead',
      'listMessages',
      'submitVideo',
      'markMessageThreadRead',
      'listMessages',
    ]);
    final request = api.requests.whereType<SubmitVideoRequest>().single;
    expect(request.coachId, Fixtures.coachId);
    expect(request.sport, Sport.tennis);

    // 依頼を出すのは開いたときの 1 回だけ（スレッドを開き直しても出さない）
    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    await tapText(tester, '担当コーチ');
    expect(find.text('動画の添削を依頼する'), findsNothing);
  });

  testWidgets('担当コーチが 2 人以上なら、「添削を依頼する」でスレッドの一覧を開く', (tester) async {
    final api = FakeStudentApi(
      entitlement: entitlement,
      threads: [
        Fixtures.thread,
        thread('other', unreadCount: 0, coachId: 'coach-b'),
      ],
    );
    final container = await pumpHome(tester, api);

    await tapText(tester, '添削を依頼する');

    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.messageThreads);
    expect(state.chatReturnStep, FlowStep.home);
    expect(state.promptVideoReviewRequest, isFalse);
  });

  testWidgets('登録後のほかの画面からは、アプリバーの「ホーム」でホームに戻って読み直す', (tester) async {
    final api = FakeStudentApi(entitlement: entitlement);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home.copyWith(step: FlowStep.slotSearch),
    );
    await tester.pumpAndSettle();
    expect(find.byTooltip('ホーム'), findsOneWidget);

    await tester.tap(find.byTooltip('ホーム'));
    await tester.pumpAndSettle();

    expect(container.read(studentFlowProvider).step, FlowStep.home);
    expect(api.calls, [
      'listMyBookings',
      'getMyEntitlement',
      'listMessageThreads',
    ]);
    // ホームの画面には「ホーム」の入口を出さない
    expect(find.byTooltip('ホーム'), findsNothing);
  });
}
