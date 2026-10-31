import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/chat.dart';
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
      'listMyVideos',
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

  testWidgets('これからの予約には、予約した時点のレッスンメニューの名前と 1 回の時間を出す。メニューのない予約は今までどおり', (
    tester,
  ) async {
    final api = FakeStudentApi(
      myBookings: [
        Fixtures.myBooking(
          bookingId: 'booking-menu',
          startsIn: const Duration(days: 3),
          lessonMenu: Fixtures.bookingLessonMenu,
        ),
        Fixtures.myBooking(
          bookingId: 'booking-async-menu',
          startsIn: const Duration(days: 2),
          coachDisplayName: null,
          lessonMenu: const BookingLessonMenu(
            lessonMenuId: '13131313-1313-4313-8313-131313131313',
            name: 'サーブの動画添削',
          ),
        ),
        Fixtures.myBooking(
          bookingId: 'booking-no-menu',
          startsIn: const Duration(days: 1),
        ),
      ],
    );
    await pumpHome(tester, api);

    expect(
      find.text('テニス・メニュー: フォアハンドの基礎（1 回 60 分）・コーチ: 佐藤・確定済み'),
      findsOneWidget,
    );
    expect(find.text('テニス・メニュー: サーブの動画添削・確定済み'), findsOneWidget);
    expect(find.text('テニス・コーチ: 佐藤・確定済み'), findsOneWidget);
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

    final threads = container.read(chatProvider.notifier).recentThreads();
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
    'listMyVideos': '返ってきた添削を読み込めませんでした。「ホームを更新する」でもう一度読み込めます。',
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
      expect(
        find.text('鈴木 コーチ'),
        operationId == 'listMyVideos' ? findsNothing : findsOneWidget,
      );
      // 読めなかったときは欄を隠さず、見出しと旨を出す
      expect(find.text('返ってきた添削'), findsOneWidget);
    });
  }

  testWidgets('返ってきた添削は、返却済みの動画を返却の新しい順に 3 件まで、コーチの名前・返却の日時・メニューで出す', (
    tester,
  ) async {
    MyVideo returned(
      String id, {
      required Duration returnedAgo,
      String? coachDisplayName = '佐藤',
      VideoLessonMenu? lessonMenu,
    }) => MyVideo(
      videoId: id,
      coachId: Fixtures.coachId,
      coachDisplayName: coachDisplayName,
      sport: Sport.tennis,
      submittedAt: Fixtures.now.subtract(const Duration(days: 30)),
      reviewStatus: VideoReviewStatus.returned,
      returnedAt: Fixtures.now.subtract(returnedAgo),
      lessonMenu: lessonMenu,
    );
    final newest = returned(
      'r-1h',
      returnedAgo: const Duration(hours: 1),
      lessonMenu: Fixtures.videoLessonMenu,
    );
    final second = returned(
      'r-1d',
      returnedAgo: const Duration(days: 1),
      coachDisplayName: null,
    );
    final third = returned(
      'r-2d',
      returnedAgo: const Duration(days: 2),
      coachDisplayName: '鈴木',
    );
    final fourth = returned('r-5d', returnedAgo: const Duration(days: 5));
    final api = FakeStudentApi(
      // 送った時刻の新しい順（契約の並び）。返却の順とは違う。添削待ちは出さない
      myVideos: [
        Fixtures.myVideo(
          videoId: 'waiting',
          submittedAgo: const Duration(minutes: 5),
        ),
        fourth,
        third,
        newest,
        second,
      ],
    );
    final container = await pumpHome(tester, api);

    expect(
      container
          .read(studentFlowProvider.notifier)
          .returnedVideos()
          .map((v) => v.videoId),
      ['r-1h', 'r-1d', 'r-2d'],
    );
    expect(find.text('返ってきた添削'), findsOneWidget);
    Finder rowWith(String title, String subtitle) => find.ancestor(
      of: find.text(subtitle),
      matching: find.widgetWithText(ListTile, title),
    );
    final rows = [
      rowWith(
        '佐藤 コーチ',
        '返却: ${formatDateTime(newest.returnedAt!)}・メニュー: サーブの動画添削',
      ),
      rowWith('担当コーチ', '返却: ${formatDateTime(second.returnedAt!)}'),
      rowWith('鈴木 コーチ', '返却: ${formatDateTime(third.returnedAt!)}'),
    ];
    for (final row in rows) {
      expect(row, findsOneWidget);
    }
    // 4 件目は出さない
    expect(
      find.text('返却: ${formatDateTime(fourth.returnedAt!)}'),
      findsNothing,
    );
    // 返却の新しい順に上から並べる
    final tops = [for (final row in rows) tester.getTopLeft(row).dy];
    expect(tops, orderedEquals([...tops]..sort()));
  });

  testWidgets('返却済みの動画がなければ、返ってきた添削の欄を出さない', (tester) async {
    final api = FakeStudentApi(
      myVideos: [
        Fixtures.myVideo(
          videoId: 'waiting',
          submittedAgo: const Duration(hours: 1),
        ),
      ],
    );
    await pumpHome(tester, api);

    expect(find.text('返ってきた添削'), findsNothing);
    expect(find.textContaining('返却:'), findsNothing);
    expect(find.textContaining('読み込めませんでした'), findsNothing);
  });

  testWidgets('返ってきた添削の行から添削結果を開き（完了記録には進めない）、閉じるとホームに戻る', (tester) async {
    final returned = Fixtures.myVideos.firstWhere(
      (v) => v.videoId == 'video-returned',
    );
    final api = FakeStudentApi(
      videoReviews: {returned.videoId: Fixtures.reviewReturnedWithAi},
    );
    final container = await pumpHome(tester, api);
    api.calls.clear();

    await tapText(tester, '鈴木 コーチ');

    expect(api.calls, ['getVideoReview']);
    expect(api.requests.last, returned.videoId);
    var state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.myVideoReview);
    expect(state.review, isNull);
    expect(find.text('添削結果'), findsOneWidget);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
    expect(find.text('完了の記録へ進む'), findsNothing);

    await tester.tap(find.byTooltip('添削結果を閉じる'));
    await tester.pumpAndSettle();
    state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.home);
    expect(find.text('返ってきた添削'), findsOneWidget);
  });

  testWidgets('返ってきた添削の添削結果を読めなければホームに留まり、失敗の文言を出す', (tester) async {
    final api = FakeStudentApi()..videoReviewFailures.add('video-returned');
    final container = await pumpHome(tester, api);

    await tapText(tester, '鈴木 コーチ');

    expect(container.read(studentFlowProvider).step, FlowStep.home);
    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
  });

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

  testWidgets('アプリバーの「チャット」に未読の数の合計のしるしを付け、スレッドを開いて既読にしてホームに戻ると消える', (
    tester,
  ) async {
    final api = FakeStudentApi(
      entitlement: entitlement,
      threads: [
        Fixtures.thread,
        thread('other', unreadCount: 2, coachId: 'coach-b'),
      ],
    );
    await pumpHome(tester, api);

    Finder chatMark(String count) =>
        find.descendant(of: find.byTooltip('チャット'), matching: find.text(count));
    expect(chatMark('3'), findsOneWidget);

    await tapText(tester, '佐藤 コーチ');
    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('チャットを閉じる'));
    await tester.pumpAndSettle();
    expect(chatMark('2'), findsOneWidget);
    expect(chatMark('3'), findsNothing);
  });

  testWidgets('未読がなければアプリバーの「チャット」にしるしを付けず、ホームの外では付けない', (tester) async {
    final api = FakeStudentApi(
      entitlement: entitlement,
      threads: [thread('read', unreadCount: 0)],
    );
    final container = await pumpHome(tester, api);
    Finder badge() => find.descendant(
      of: find.byTooltip('チャット'),
      matching: find.byWidgetPredicate((w) => w is Badge && w.isLabelVisible),
    );
    expect(badge(), findsNothing);

    api.threads[0] = thread('read', unreadCount: 1);
    await container.read(studentFlowProvider.notifier).refreshHome();
    await tester.pumpAndSettle();
    expect(badge(), findsOneWidget);

    // ほかの画面ではスレッドを読み直さないので出さない
    await tapText(tester, 'スロットを探して予約する');
    expect(container.read(studentFlowProvider).step, FlowStep.slotSearch);
    expect(find.byTooltip('チャット'), findsOneWidget);
    expect(badge(), findsNothing);
  });

  testWidgets('やり取りのスレッドを押すとそのチャットを開き（既読にする）、チャットを閉じるとホームに戻る', (tester) async {
    final api = FakeStudentApi(entitlement: entitlement);
    final container = await pumpHome(tester, api);
    api.calls.clear();

    await tapText(tester, '佐藤 コーチ');

    expect(find.text('担当コーチとのチャット'), findsOneWidget);
    expect(api.calls, ['markMessageThreadRead', 'listMessages']);
    expect(container.read(chatProvider).returnStep, FlowStep.home);

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
    expect(container.read(chatProvider).promptVideoReviewRequest, isFalse);
    await tapText(tester, '動画を選ぶ（モック）');
    await tapText(tester, '依頼する');

    expect(api.calls, [
      'markMessageThreadRead',
      'listMessages',
      'listCoachLessonMenus',
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

    expect(container.read(studentFlowProvider).step, FlowStep.messageThreads);
    final chat = container.read(chatProvider);
    expect(chat.returnStep, FlowStep.home);
    expect(chat.promptVideoReviewRequest, isFalse);
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
      'listMyVideos',
    ]);
    // ホームの画面には「ホーム」の入口を出さない
    expect(find.byTooltip('ホーム'), findsNothing);
  });
}
