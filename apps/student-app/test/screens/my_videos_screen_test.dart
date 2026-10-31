import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/ui/labels.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final afterSubscription = StudentFlowState(
    step: FlowStep.subscription,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
    subscription: Fixtures.subscriptionStarted.subscription,
    entitlement: Fixtures.subscriptionStarted.entitlement,
  );

  const openReviewLabel = '添削結果を見る';

  MyVideo videoOf(String videoId) =>
      Fixtures.myVideos.firstWhere((v) => v.videoId == videoId);

  /// 一覧の 1 件（送った日時の行で探す）
  Finder cardOf(MyVideo item) =>
      find.widgetWithText(Card, '${formatDateTime(item.submittedAt)} に送信');

  Finder inCard(MyVideo item, String text) =>
      find.descendant(of: cardOf(item), matching: find.text(text));

  Future<void> openVideos(WidgetTester tester) async {
    await tester.tap(find.byTooltip('自分の動画'));
    await tester.pumpAndSettle();
  }

  testWidgets(
    'アプリバーから開くと listMyVideos で読み込み、送った日時・スポーツ・メニュー・コーチ・状態を出し、閉じると開いた画面に戻る',
    (tester) async {
      final api = FakeStudentApi();
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: afterSubscription,
      );

      await openVideos(tester);

      expect(api.calls, ['listMyVideos']);
      expect(find.text('自分の動画'), findsOneWidget);
      expect(find.byType(Card), findsNWidgets(2));
      final waiting = videoOf('video-waiting');
      final returned = videoOf('video-returned');
      for (final (item, texts) in [
        (waiting, ['テニス', 'メニュー: サーブの動画添削', 'コーチ: 佐藤', '状態: 添削待ち']),
        (returned, ['サッカー', 'コーチ: 鈴木', '状態: 返却済み']),
      ]) {
        expect(cardOf(item), findsOneWidget, reason: item.videoId);
        for (final text in texts) {
          expect(inCard(item, text), findsOneWidget, reason: text);
        }
      }
      // 送った時刻の新しい順のまま並べる
      expect(
        tester.getTopLeft(cardOf(waiting)).dy,
        lessThan(tester.getTopLeft(cardOf(returned)).dy),
      );
      // メニューを選ばずに送った動画には、メニューを出さない
      expect(
        find.descendant(
          of: cardOf(returned),
          matching: find.textContaining('メニュー:'),
        ),
        findsNothing,
      );
      // 返却済みの動画にだけ、添削結果を開くボタン
      expect(inCard(waiting, openReviewLabel), findsNothing);
      expect(inCard(returned, openReviewLabel), findsOneWidget);
      // 自分の動画の画面からはほかの脇の画面を重ねて開かない
      expect(find.byTooltip('自分の予約'), findsNothing);
      expect(find.byTooltip('チャット'), findsNothing);
      expect(find.byTooltip('通知の設定'), findsNothing);

      await tester.tap(find.byTooltip('自分の動画を閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.subscription);
    },
  );

  testWidgets('コーチの名前がない動画は名前の行を出さず、動画がなければそう伝える', (tester) async {
    final api = FakeStudentApi(
      myVideos: [
        Fixtures.myVideo(
          videoId: 'video-no-coach',
          submittedAgo: const Duration(hours: 1),
          coachDisplayName: null,
        ),
      ],
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
    );
    await openVideos(tester);

    expect(find.textContaining('コーチ:'), findsNothing);
    expect(find.text('テニス'), findsOneWidget);
    expect(find.text('まだ動画を送っていません'), findsNothing);

    api.myVideos.clear();
    await tapText(tester, '一覧を更新する');
    expect(api.calls, ['listMyVideos', 'listMyVideos']);
    expect(find.byType(Card), findsNothing);
    expect(find.text('まだ動画を送っていません'), findsOneWidget);
  });

  testWidgets('読み込みに失敗したら、ほかの画面と同じ文言を出し、空の一覧とは言わない', (tester) async {
    final api = FakeStudentApi(
      failures: {'listMyVideos': Fixtures.unreachable},
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
    );
    await openVideos(tester);

    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    expect(find.text('まだ動画を送っていません'), findsNothing);
    expect(find.text('自分の動画'), findsOneWidget);
  });

  testWidgets('返却済みの行から getVideoReview でその動画の添削結果を開き、完了記録には進めず、閉じると一覧に戻る', (
    tester,
  ) async {
    final returned = videoOf('video-returned');
    final api = FakeStudentApi(
      videoReviews: {returned.videoId: Fixtures.reviewReturnedWithAi},
    );
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
    );
    await openVideos(tester);

    final button = inCard(returned, openReviewLabel);
    await tester.ensureVisible(button);
    await tester.tap(button);
    await tester.pumpAndSettle();

    expect(api.calls, ['listMyVideos', 'getVideoReview']);
    expect(api.requests.single, returned.videoId);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.myVideoReview);
    // 一周シナリオの添削（state.review）は書き換えない
    expect(state.review, isNull);
    expect(find.text('添削結果'), findsOneWidget);
    expect(find.text('コーチの添削'), findsOneWidget);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
    expect(find.text('AI の一次フィードバック'), findsOneWidget);
    expect(find.text('動画を送りました。'), findsNothing);
    expect(find.text('完了の記録へ進む'), findsNothing);
    expect(find.byTooltip('チャット'), findsNothing);

    await tester.tap(find.byTooltip('添削結果を閉じる'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.myVideos);
    expect(cardOf(returned), findsOneWidget);

    await tester.tap(find.byTooltip('自分の動画を閉じる'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  testWidgets('添削結果を読めなければ一覧に留まり、失敗の文言を出す', (tester) async {
    final returned = videoOf('video-returned');
    final api = FakeStudentApi()..videoReviewFailures.add(returned.videoId);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
    );
    await openVideos(tester);

    final button = inCard(returned, openReviewLabel);
    await tester.ensureVisible(button);
    await tester.tap(button);
    await tester.pumpAndSettle();

    expect(container.read(studentFlowProvider).step, FlowStep.myVideos);
    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    expect(cardOf(returned), findsOneWidget);
  });
}
