import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/state/student_flow.dart';

import 'support/fake_local_media.dart';
import 'support/fake_student_api.dart';
import 'support/pump.dart';

/// 一周シナリオ（体験予約 → 初回レッスン → 完了記録 → 評価 → 月額契約）を画面遷移でたどる。
/// 今の時刻は端末の時計のまま（見本のスロットの開始時刻より後なので、入室前チェックを開けて入室できる）。端末のカメラ・マイクは作り物。
void main() {
  Future<void> pumpApp(WidgetTester tester, FakeStudentApi api) =>
      tester.pumpWidget(
        ProviderScope(
          overrides: [
            studentApiProvider.overrideWithValue(api),
            localMediaProvider.overrideWithValue(FakeLocalMedia()),
          ],
          child: const StudentApp(),
        ),
      );

  Future<void> registerAndBook(WidgetTester tester) async {
    expect(find.text('アカウント登録'), findsOneWidget);
    await tester.enterText(
      find.widgetWithText(TextFormField, 'メールアドレス'),
      'yamada@example.com',
    );
    await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
    await tapText(tester, 'テニス');
    await tapText(tester, '登録する');

    expect(find.text('ホーム'), findsOneWidget);
    expect(find.text('まだこれからの予約はありません。'), findsOneWidget);
    await tapText(tester, 'スロットを探して予約する');
    expect(find.text('スロットを探す'), findsOneWidget);
    await tapText(tester, '体験予約する');

    expect(find.text('予約の確定待ち'), findsOneWidget);
    await tapText(tester, '予約を確定する');
    expect(find.text('予約が確定しました'), findsOneWidget);
  }

  Future<void> recordAndSubscribe(WidgetTester tester) async {
    expect(find.text('レッスンの完了'), findsOneWidget);
    await tapText(tester, '完了を記録する');
    expect(find.text('初回レッスンを完了しました'), findsOneWidget);
    await tapText(tester, 'レッスンを評価する');

    expect(find.text('レッスンの評価'), findsOneWidget);
    await tapText(tester, '5　とてもよかった');
    await tester.enterText(find.byType(TextField), 'わかりやすかったです');
    await tester.pump();
    await tapText(tester, '評価を送る');
    expect(find.text('評価を送りました'), findsOneWidget);
    await tapText(tester, '月額契約へ進む');

    expect(find.text('月額契約'), findsOneWidget);
    await tapText(tester, '月額契約を始める');
    expect(find.text('月額契約を開始しました'), findsOneWidget);
  }

  testWidgets('通話で受ける一周: 登録 → 検索 → 体験予約 → 確定 → 通話 → 完了記録 → 評価 → 月額契約', (
    tester,
  ) async {
    // 登録したばかりの生徒（予約がないので体験予約できる）
    final api = FakeStudentApi(myBookings: const []);
    await pumpApp(tester, api);

    await registerAndBook(tester);
    await tapText(tester, 'ビデオ通話で受ける');
    expect(find.text('入室前チェック'), findsOneWidget);
    await tapText(tester, '入室する');
    expect(find.text('ビデオ通話'), findsOneWidget);
    await tapText(tester, '通話を終了する');
    await recordAndSubscribe(tester);

    expect(api.calls, [
      'registerAccount',
      'listMyBookings',
      'getMyEntitlement',
      'listMessageThreads',
      'listMyVideos',
      'listMyNotifications',
      'listFavoriteCoaches',
      'listMyLessonCompletions',
      'getMyNextActions',
      'listMyBookings',
      'searchSlots',
      'getCoachReviewSummary',
      'createBooking',
      'confirmBooking',
      'startLiveLessonSession',
      'endLiveLessonSession',
      'recordLessonCompletion',
      'submitLessonReview',
      'listSubscriptionPlans',
      'startSubscription',
    ]);
  });

  testWidgets(
    '動画で受ける一周: 登録 → 検索 → 体験予約 → 確定 → 動画の送付 → 添削結果 → 完了記録 → 評価 → 月額契約',
    (tester) async {
      final api = FakeStudentApi(
        myBookings: const [],
        reviews: [Fixtures.reviewInProgress, Fixtures.reviewReturned],
      );
      await pumpApp(tester, api);

      await registerAndBook(tester);
      await tapText(tester, '動画を送って添削を受ける');
      expect(find.text('動画を送る'), findsOneWidget);
      await tapText(tester, '動画を選ぶ（モック）');
      await tapText(tester, '送付する');

      expect(find.text('添削結果'), findsOneWidget);
      expect(find.text('コーチが添削しています。返却されるまでお待ちください。'), findsOneWidget);
      // コーチが添削を返した（サーバーの次の操作に、その動画の完了の記録が出る）
      api.nextActions.add(Fixtures.nextRecordAsyncCompletion);
      await tapText(tester, '添削結果を確かめる');
      expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
      await tapText(tester, '完了の記録へ進む');
      await recordAndSubscribe(tester);

      expect(api.calls, [
        'registerAccount',
        'listMyBookings',
        'getMyEntitlement',
        'listMessageThreads',
        'listMyVideos',
        'listMyNotifications',
        'listFavoriteCoaches',
        'listMyLessonCompletions',
        'getMyNextActions',
        'listMyBookings',
        'searchSlots',
        'getCoachReviewSummary',
        'createBooking',
        'confirmBooking',
        'listCoachLessonMenus',
        'submitVideo',
        'getVideoReview',
        'getVideoReview',
        'getMyNextActions',
        'recordLessonCompletion',
        'submitLessonReview',
        'listSubscriptionPlans',
        'startSubscription',
      ]);
    },
  );

  testWidgets(
    '途中で離れる一周: 登録 → 体験予約 → 自分の予約から確定 → ホームの次にやることからレッスン → 完了の記録 → 今月のビデオ通話から月額契約',
    (tester) async {
      // 今の時刻はテストで進める（見本のスロットの 2 日前から始める。確定待ちの予約は始まる前だけ確定できる）
      var now = Fixtures.now;
      final api = FakeStudentApi(myBookings: const []);
      tester.view.physicalSize = const Size(800, 3000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            studentApiProvider.overrideWithValue(api),
            localMediaProvider.overrideWithValue(FakeLocalMedia()),
            clockProvider.overrideWithValue(() => now),
          ],
          child: const StudentApp(),
        ),
      );
      Future<void> goHome() async {
        await tester.tap(find.byTooltip('ホーム'));
        await tester.pumpAndSettle();
        expect(find.text('ホーム'), findsOneWidget);
      }

      // 登録して体験予約を作り、確定せずにホームへ離れる
      expect(find.text('アカウント登録'), findsOneWidget);
      await tester.enterText(
        find.widgetWithText(TextFormField, 'メールアドレス'),
        'yamada@example.com',
      );
      await tester.enterText(find.widgetWithText(TextFormField, '表示名'), '山田');
      await tapText(tester, 'テニス');
      await tapText(tester, '登録する');
      await tapText(tester, 'スロットを探して予約する');
      await tapText(tester, '体験予約する');
      expect(find.text('予約の確定待ち'), findsOneWidget);
      api.nextActions
        ..clear()
        ..add(Fixtures.nextConfirmBooking);
      await goHome();
      expect(find.text('体験予約がまだ確定していません'), findsOneWidget);

      // 自分の予約の確定待ちから確定する
      await tester.tap(find.byTooltip('自分の予約'));
      await tester.pumpAndSettle();
      await tapText(tester, '内容を確かめて確定する');
      expect(find.text('予約の確定待ち'), findsOneWidget);
      await tapText(tester, '予約を確定する');
      expect(find.text('予約が確定しました'), findsOneWidget);
      expect(api.calls, contains('confirmBooking'));

      // 受け方を選ばずにホームへ離れ、開始時刻を過ぎてから次にやることのビデオ通話で受ける
      api.nextActions
        ..clear()
        ..add(Fixtures.nextJoinLiveLesson);
      await goHome();
      expect(find.text('体験予約のレッスンを受けましょう'), findsOneWidget);
      now = Fixtures.slot.startsAt.add(const Duration(minutes: 1));
      await tester.pump(const Duration(seconds: 1));
      await tapText(tester, 'ビデオ通話で受ける');
      expect(find.text('入室前チェック'), findsOneWidget);
      await tapText(tester, '入室する');
      expect(find.text('ビデオ通話'), findsOneWidget);
      await tapText(tester, '通話を終了する');
      expect(find.text('レッスンの完了'), findsOneWidget);

      // 完了を記録せずにホームへ離れ、次にやることから完了を記録する
      api.nextActions
        ..clear()
        ..add(Fixtures.nextRecordLiveCompletion);
      await goHome();
      expect(find.text('ビデオ通話が終わりました'), findsOneWidget);
      await tapText(tester, '完了の記録へ進む');
      expect(find.text('レッスンの完了'), findsOneWidget);
      await tapText(tester, '完了を記録する');
      expect(find.text('初回レッスンを完了しました'), findsOneWidget);
      expect(
        (api.requests.whereType<RecordLessonCompletionRequest>().single)
            .toJson()['session_id'],
        Fixtures.sessionId,
      );

      // 月額契約へ進まずにホームへ離れる。プロフィールにも「月額契約へ進む」がある
      api.nextActions
        ..clear()
        ..add(Fixtures.nextStartSubscription);
      await goHome();
      expect(find.text('月額契約で、続けて教わりましょう'), findsOneWidget);
      await tester.tap(find.byTooltip('プロフィール'));
      await tester.pumpAndSettle();
      expect(find.text('月額契約へ進む'), findsOneWidget);
      expect(find.text('まだ月額契約はありません'), findsOneWidget);
      await tester.tap(find.byTooltip('プロフィールを閉じる'));
      await tester.pumpAndSettle();

      // 今月のビデオ通話の欄の文字のボタンから月額契約を始める
      final quotaEntry = find.widgetWithText(TextButton, '月額契約へ進む');
      await tester.ensureVisible(quotaEntry);
      await tester.tap(quotaEntry);
      await tester.pumpAndSettle();
      await tapText(tester, '月額契約を始める');
      expect(find.text('月額契約を開始しました'), findsOneWidget);

      // 契約したあとは、ホームにもプロフィールにも「月額契約へ進む」を出さない
      api.nextActions.clear();
      await goHome();
      expect(find.text('次にやること'), findsNothing);
      expect(find.text('月額契約へ進む'), findsNothing);
      expect(find.text('月 2 回のうち残り 2 回'), findsOneWidget);
      await tester.tap(find.byTooltip('プロフィール'));
      await tester.pumpAndSettle();
      expect(find.text('月額契約へ進む'), findsNothing);
      expect(find.textContaining('プラン: スタンダード・'), findsOneWidget);
      expect(api.calls.where((c) => c == 'startSubscription'), hasLength(1));
    },
  );

  testWidgets('チャット: 月額契約の後 → スレッドの一覧 → メッセージ（既読にする）→ 送信 → 一覧 → 閉じて元の画面', (
    tester,
  ) async {
    // 登録したばかりの生徒（予約がないので体験予約できる）
    final api = FakeStudentApi(myBookings: const []);
    await pumpApp(tester, api);

    expect(find.byTooltip('チャット'), findsNothing);
    await registerAndBook(tester);
    await tapText(tester, 'ビデオ通話で受ける');
    await tapText(tester, '入室する');
    await tapText(tester, '通話を終了する');
    await recordAndSubscribe(tester);
    api.calls.clear();

    await tester.tap(find.byTooltip('チャット'));
    await tester.pumpAndSettle();
    expect(find.text('チャット'), findsOneWidget);
    expect(find.text('未読 1'), findsOneWidget);

    await tapText(tester, '担当コーチ');
    expect(find.text('担当コーチとのチャット'), findsOneWidget);
    expect(find.text('体験レッスンでお待ちしています'), findsOneWidget);

    await tester.enterText(find.byType(TextField), '次回もお願いします');
    await tester.pump();
    await tapText(tester, '送信する');
    expect(find.text('次回もお願いします'), findsOneWidget);

    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    expect(find.text('自分: 次回もお願いします'), findsOneWidget);
    expect(find.text('未読 1'), findsNothing);

    await tester.tap(find.byTooltip('チャットを閉じる'));
    await tester.pumpAndSettle();
    expect(find.text('月額契約を開始しました'), findsOneWidget);

    expect(api.calls, [
      'listMessageThreads',
      'markMessageThreadRead',
      'listMessages',
      'sendMessage',
      'listMessageThreads',
    ]);
  });

  test('自分の予約は copyWith で読み取り専用にして持ち、外から書き換えられない', () {
    final source = [...Fixtures.myBookings];
    final state = StudentFlowState(step: FlowStep.myBookings)
        .copyWith(myBookings: source);
    source.clear();
    expect(state.myBookings, hasLength(Fixtures.myBookings.length));
    expect(
      () => state.myBookings!.add(Fixtures.myBookings.first),
      throwsUnsupportedError,
    );
  });

  test('自分の動画は、コンストラクタで直接渡しても copyWith で渡しても写して持ち、外から書き換えられない', () {
    // コンストラクタで直接渡した元のリストを後で変えても、状態は変わらない
    final direct = [...Fixtures.myVideos];
    final constructed = StudentFlowState(
      step: FlowStep.myVideos,
      myVideos: direct,
    );
    direct.clear();
    expect(constructed.myVideos, Fixtures.myVideos);
    expect(
      () => constructed.myVideos!.add(Fixtures.myVideos.first),
      throwsUnsupportedError,
    );
    expect(() => constructed.myVideos!.clear(), throwsUnsupportedError);
    // myVideos を渡さない copyWith でも、元のリストとのつながりは次の状態に残らない
    final next = constructed.copyWith(busy: true);
    direct.addAll(Fixtures.myVideos);
    expect(next.myVideos, Fixtures.myVideos);

    // copyWith で渡した元のリストを後で変えても、状態は変わらない
    final source = [...Fixtures.myVideos];
    final copied = StudentFlowState(step: FlowStep.myVideos)
        .copyWith(myVideos: source);
    source.clear();
    expect(copied.myVideos, Fixtures.myVideos);
    expect(() => copied.myVideos!.clear(), throwsUnsupportedError);
    // 渡さなければ未取得のまま
    expect(StudentFlowState().myVideos, isNull);
  });
}
