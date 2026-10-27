import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/app.dart';
import 'package:rally_student_app/state/student_flow.dart';

import 'support/fake_student_api.dart';
import 'support/pump.dart';

/// 一周シナリオ（体験予約 → 初回レッスン → 完了記録 → 評価 → 月額契約）を画面遷移でたどる。
void main() {
  Future<void> pumpApp(WidgetTester tester, FakeStudentApi api) =>
      tester.pumpWidget(
        ProviderScope(
          overrides: [studentApiProvider.overrideWithValue(api)],
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
    await tapText(tester, '5');
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
    final api = FakeStudentApi();
    await pumpApp(tester, api);

    await registerAndBook(tester);
    await tapText(tester, 'ビデオ通話で受ける');
    expect(find.text('ビデオ通話'), findsOneWidget);
    await tapText(tester, '通話を開始する');
    await tapText(tester, '通話を終了する');
    await recordAndSubscribe(tester);

    expect(api.calls, [
      'registerAccount',
      'searchSlots',
      'createBooking',
      'confirmBooking',
      'startLiveLessonSession',
      'endLiveLessonSession',
      'recordLessonCompletion',
      'submitLessonReview',
      'startSubscription',
    ]);
  });

  testWidgets(
    '動画で受ける一周: 登録 → 検索 → 体験予約 → 確定 → 動画の送付 → 添削結果 → 完了記録 → 評価 → 月額契約',
    (tester) async {
      final api = FakeStudentApi(
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
      await tapText(tester, '添削結果を確かめる');
      expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
      await tapText(tester, '完了の記録へ進む');
      await recordAndSubscribe(tester);

      expect(api.calls, [
        'registerAccount',
        'searchSlots',
        'createBooking',
        'confirmBooking',
        'submitVideo',
        'getVideoReview',
        'getVideoReview',
        'recordLessonCompletion',
        'submitLessonReview',
        'startSubscription',
      ]);
    },
  );

  testWidgets('チャット: 月額契約の後 → スレッドの一覧 → メッセージ（既読にする）→ 送信 → 一覧 → 閉じて元の画面', (
    tester,
  ) async {
    final api = FakeStudentApi();
    await pumpApp(tester, api);

    expect(find.byTooltip('チャット'), findsNothing);
    await registerAndBook(tester);
    await tapText(tester, 'ビデオ通話で受ける');
    await tapText(tester, '通話を開始する');
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
      'listMessages',
      'markMessageThreadRead',
      'sendMessage',
      'listMessageThreads',
    ]);
  });
}
