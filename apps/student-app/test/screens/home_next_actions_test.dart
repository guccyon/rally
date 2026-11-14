import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/lesson_completions.dart';
import 'package:rally_student_app/state/next_actions.dart';
import 'package:rally_student_app/state/prejoin_check.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/ui/components/rally_button.dart';
import 'package:rally_student_app/ui/components/rally_card.dart';
import 'package:rally_student_app/ui/labels.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

/// ホームの「次にやること」（`getMyNextActions`。ADR 0010）。サーバーが返した順のまま出し、続きの画面は ID で読み直して開く。
void main() {
  final home = StudentFlowState(
    step: FlowStep.home,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  /// 確定待ちの体験予約（[Fixtures.nextConfirmBooking] の予約）が自分の予約にある
  MyBooking pendingTrial({BookingStatus status = BookingStatus.pending}) {
    final item = Fixtures.myBooking(
      bookingId: Fixtures.bookingId,
      startsIn: Fixtures.slot.startsAt.difference(Fixtures.now),
      status: status,
      coachDisplayName: '山田 花子',
    );
    return MyBooking(
      booking: Fixtures.bookingWith(
        Fixtures.withLessonMenu(item.booking, Fixtures.bookingLessonMenu),
        status: status,
      ),
      slot: item.slot,
    );
  }

  /// ホームを描いて欄を読む（アプリの段には予約も動画も持っていない。開き直したのと同じ）
  Future<ProviderContainer> pumpHome(
    WidgetTester tester,
    FakeStudentApi api, {
    DateTime? now,
    DateTime Function()? clock,
  }) async {
    tester.view.physicalSize = const Size(800, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
      now: now,
      clock: clock,
    );
    await container.read(studentFlowProvider.notifier).refreshHome();
    await tester.pumpAndSettle();
    return container;
  }

  Finder card() => find.byType(RallyCard);

  testWidgets('確定待ちの体験予約を、あいさつのすぐ下のカードで出し、予約の ID で読み直した確定待ちの画面を開く', (
    tester,
  ) async {
    final api = FakeStudentApi(
      myBookings: [pendingTrial()],
      nextActions: [Fixtures.nextConfirmBooking],
    );
    final container = await pumpHome(tester, api);

    expect(find.text('次にやること'), findsOneWidget);
    // あいさつの下・ほかの欄の上
    final top = tester.getTopLeft(find.text('次にやること')).dy;
    expect(top, greaterThan(tester.getTopLeft(find.text('山田さん、こんにちは')).dy));
    expect(top, lessThan(tester.getTopLeft(find.text('これからの予約')).dy));
    expect(
      find.descendant(of: card(), matching: find.text('確定待ち')),
      findsOneWidget,
    );
    expect(find.text('体験予約がまだ確定していません'), findsOneWidget);
    expect(
      find.text(formatTimeRange(Fixtures.slot.startsAt, Fixtures.slot.endsAt)),
      findsOneWidget,
    );
    // 同じ予約は「これからの予約」にも並ぶ（ここは一覧として残す）
    expect(
      find.descendant(
        of: card(),
        matching: find.text('テニス・メニュー: フォアハンドの基礎（1 回 60 分）・コーチ: 山田 花子'),
      ),
      findsOneWidget,
    );
    expect(
      find.text('確定するまで、このスロットは押さえたままになり、ほかのスロットで体験予約を取り直すこともできません。'),
      findsOneWidget,
    );
    api.calls.clear();

    await tapText(tester, '内容を確かめて確定する');

    // 確定はこの画面では呼ばない（中身を見せた画面で押す）
    expect(api.calls, ['listMyBookings']);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.bookingPending);
    expect(state.booking?.bookingId, Fixtures.bookingId);
    expect(find.text('予約の確定待ち'), findsOneWidget);
    expect(find.text('予約を確定する'), findsOneWidget);
    expect(find.text('この予約をキャンセルする'), findsOneWidget);
  });

  testWidgets('サーバーが返した順のまま、1 件目をカード、2 件目からを一覧の行で出し、行からも同じ先へ進む', (
    tester,
  ) async {
    final api = FakeStudentApi(
      // サーバーの優先の順（アプリの考える順と違っても並べ替えない）
      nextActions: [
        Fixtures.nextStartSubscription,
        Fixtures.nextRecordLiveCompletion,
        Fixtures.nextVideoReviewInProgress,
      ],
    );
    final container = await pumpHome(tester, api);

    expect(find.text('月額契約で、続けて教わりましょう'), findsOneWidget);
    expect(
      find.descendant(of: card(), matching: find.text('月額契約へ進む')),
      findsOneWidget,
    );
    final completionRow = find.widgetWithText(ListTile, '完了の記録へ進む');
    final videoRow = find.widgetWithText(ListTile, '送った動画を見る');
    expect(completionRow, findsOneWidget);
    expect(videoRow, findsOneWidget);
    expect(
      tester.getTopLeft(completionRow).dy,
      lessThan(tester.getTopLeft(videoRow).dy),
    );
    expect(find.text('コーチ: 山田 花子'), findsNWidgets(2));

    await tester.tap(completionRow);
    await tester.pumpAndSettle();

    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.lessonCompletion);
    expect(
      (state.completing! as LiveLessonCompletion).sessionId,
      Fixtures.sessionId,
    );
    expect(find.text('通話が終わりました。'), findsOneWidget);
  });

  testWidgets('次の操作が 0 件なら欄ごと出さない', (tester) async {
    await pumpHome(tester, FakeStudentApi());

    expect(find.text('次にやること'), findsNothing);
    expect(find.byType(RallyCard), findsNothing);
    expect(find.text('これからの予約'), findsOneWidget);
  });

  testWidgets('読めなければ欄に旨を出し、ほかの欄はそのまま出す。更新して読めたら差し替える', (tester) async {
    final api = FakeStudentApi(
      failures: {'getMyNextActions': Fixtures.unreachable},
    );
    final container = await pumpHome(tester, api);

    expect(find.text('次にやること'), findsOneWidget);
    expect(
      find.text('次にやることを読み込めませんでした。「ホームを更新する」でもう一度読み込めます。'),
      findsOneWidget,
    );
    // 失敗の文言（画面の上の知らせ）は出さず、ほかの欄は出す
    expect(container.read(studentFlowProvider).errorMessage, isNull);
    expect(find.text('これからの予約'), findsOneWidget);
    expect(find.text('今月のビデオ通話'), findsOneWidget);
  });

  testWidgets('最初に読んでいるあいだは見出しと読み込み中を出す', (tester) async {
    final gate = Completer<void>();
    final api = FakeStudentApi()
      ..nextActions.add(Fixtures.nextStartSubscription)
      ..nextActionsGate = gate;
    tester.view.physicalSize = const Size(800, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
    );
    unawaited(container.read(studentFlowProvider.notifier).refreshHome());
    await tester.pump();

    expect(find.text('次にやること'), findsOneWidget);
    expect(find.text('次にやることを読み込んでいます…'), findsOneWidget);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('次にやることを読み込んでいます…'), findsNothing);
    expect(find.text('月額契約で、続けて教わりましょう'), findsOneWidget);
  });

  testWidgets('確定済みの予約は開始 5 分前まで「ビデオ通話で受ける」を押せず、開いたまま 5 分前になると押せて入室前チェックを開く', (
    tester,
  ) async {
    var now = prejoinCheckOpensAt(Fixtures.slot.startsAt)
        .subtract(const Duration(seconds: 2));
    final api = FakeStudentApi(
      myBookings: [pendingTrial(status: BookingStatus.confirmed)],
      nextActions: [Fixtures.nextJoinLiveLesson],
    );
    final container = await pumpHome(tester, api, clock: () => now);

    RallyButton callButton() => tester.widget<RallyButton>(
      find.widgetWithText(RallyButton, 'ビデオ通話で受ける'),
    );
    expect(find.text('体験予約のレッスンを受けましょう'), findsOneWidget);
    expect(callButton().onPressed, isNull);
    expect(find.textContaining('から入室前チェックができます（開始 5 分前から）。'), findsOneWidget);
    // 体験予約なので動画の送付も選べる（文字のボタン）
    expect(
      tester
          .widget<RallyButton>(find.widgetWithText(RallyButton, '動画を送って添削を受ける'))
          .variant,
      RallyButtonVariant.text,
    );

    now = prejoinCheckOpensAt(Fixtures.slot.startsAt);
    await tester.pump(const Duration(seconds: 1));
    expect(callButton().onPressed, isNotNull);
    expect(find.textContaining('入室前チェックでカメラとマイクを確かめてから'), findsOneWidget);
    api.calls.clear();

    await tapText(tester, 'ビデオ通話で受ける');

    expect(api.calls, ['listMyBookings']);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.prejoinCheck);
    expect(state.booking?.bookingId, Fixtures.bookingId);
    expect(state.bookedSlot?.slotId, 'slot-${Fixtures.bookingId}');
  });

  testWidgets('添削待ちは押して進む操作がないので primary を出さず、「送った動画を見る」で添削結果を読むだけで開く', (
    tester,
  ) async {
    final api = FakeStudentApi(
      nextActions: [Fixtures.nextVideoReviewInProgress],
      videoReviews: {Fixtures.videoId: Fixtures.reviewInProgress},
    );
    final container = await pumpHome(tester, api);

    expect(find.text('コーチが添削しています'), findsOneWidget);
    expect(find.text('添削が返ってくると、ここから完了の記録へ進めます。'), findsOneWidget);
    final buttons = tester.widgetList<RallyButton>(
      find.descendant(of: card(), matching: find.byType(RallyButton)),
    );
    expect(buttons.map((b) => b.variant), [RallyButtonVariant.text]);
    api.calls.clear();

    await tapText(tester, '送った動画を見る');

    expect(api.calls, ['getVideoReview']);
    expect(container.read(studentFlowProvider).step, FlowStep.myVideoReview);
    expect(find.text('コーチが添削しています。返却されるまでお待ちください。'), findsOneWidget);
    expect(find.text('完了の記録へ進む'), findsNothing);
  });

  testWidgets('添削が返った動画は「完了の記録へ進む」で、その動画の ID を渡してレッスンの完了を開く', (tester) async {
    final api = FakeStudentApi(
      nextActions: [Fixtures.nextRecordAsyncCompletion],
    );
    final container = await pumpHome(tester, api);

    expect(find.text('添削が返ってきました'), findsOneWidget);
    expect(find.text('添削を読んだら、レッスンの完了を記録しましょう。'), findsOneWidget);

    await tapText(tester, '完了の記録へ進む');
    await tapText(tester, '完了を記録する');

    final body = api.requests.last as AsyncLessonCompletion;
    expect(body.videoId, Fixtures.videoId);
    expect(container.read(studentFlowProvider).lesson, isNotNull);
  });

  testWidgets('月額契約がなければ「月額契約へ進む」で月額契約の画面を開く', (tester) async {
    final container = await pumpHome(
      tester,
      FakeStudentApi(nextActions: [Fixtures.nextStartSubscription]),
    );

    await tapText(tester, '月額契約へ進む');

    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  testWidgets('「次のレッスンを予約する」と「添削を依頼する」は secondary（主なボタンは次にやることの 1 つ）', (
    tester,
  ) async {
    final api = FakeStudentApi(nextActions: [Fixtures.nextStartSubscription]);
    final container = await pumpHome(tester, api);
    await container.read(lessonCompletionsProvider.notifier).loadForHome();
    await tester.pumpAndSettle();

    final variants = {
      for (final button in tester.widgetList<RallyButton>(
        find.byType(RallyButton),
      ))
        button.label: button.variant,
    };
    expect(variants['次のレッスンを予約する'], RallyButtonVariant.secondary);
    expect(variants['添削を依頼する'], RallyButtonVariant.secondary);
    expect(
      variants.values.where((v) => v == RallyButtonVariant.primary),
      hasLength(1),
    );
    expect(variants['月額契約へ進む'], RallyButtonVariant.primary);
  });

  testWidgets('アプリの段の状態を捨てて開き直しても、サーバーの記録から同じ次の操作を出す', (tester) async {
    final api = FakeStudentApi(
      myBookings: [pendingTrial()],
      nextActions: [Fixtures.nextConfirmBooking],
    );
    await pumpHome(tester, api);
    expect(find.text('体験予約がまだ確定していません'), findsOneWidget);

    // 同じ生徒のまま、段の状態も次の操作の状態も新しく作り直す（開き直し）
    await tester.pumpWidget(const SizedBox());
    await pumpHome(tester, api);

    expect(find.text('体験予約がまだ確定していません'), findsOneWidget);
    expect(find.text('内容を確かめて確定する'), findsOneWidget);
  });

  test('次の操作の状態は、その動画の完了の記録があるときだけ completesVideo', () {
    expect(NextActionsState().completesVideo(Fixtures.videoId), isFalse);
    expect(
      NextActionsState(actions: [Fixtures.nextRecordAsyncCompletion])
          .completesVideo(Fixtures.videoId),
      isTrue,
    );
    expect(
      NextActionsState(actions: [Fixtures.nextVideoReviewInProgress])
          .completesVideo(Fixtures.videoId),
      isFalse,
    );
  });
}
