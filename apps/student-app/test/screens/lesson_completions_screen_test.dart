import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/lesson_completions.dart';
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

  /// ホームを描き、「受けたレッスン」で開く（応答を待つかは [settle]）。3 件の行が一度に描けるよう画面を縦に広げる
  Future<ProviderContainer> openLessonCompletions(
    WidgetTester tester,
    FakeStudentApi api, {
    bool settle = true,
  }) async {
    tester.view.physicalSize = const Size(800, 2400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
    );
    final button = find.text('受けたレッスン');
    await tester.ensureVisible(button);
    await tester.tap(button);
    if (settle) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
    }
    return container;
  }

  /// 一覧の行（カード）ごとの文言を、上から順に
  List<List<String>> rows(WidgetTester tester) => [
    for (final card in find.byType(Card).evaluate())
      [
        for (final text
            in find
                .descendant(
                  of: find.byWidget(card.widget),
                  matching: find.byType(Text),
                )
                .evaluate())
          (text.widget as Text).data ?? '',
      ],
  ];

  /// [lessonId] の行のカード
  Finder rowOf(String lessonId) {
    final completion = Fixtures.lessonCompletions.firstWhere(
      (c) => c.lessonId == lessonId,
    );
    return find.ancestor(
      of: find.text(formatDateTime(completion.completedAt)),
      matching: find.byType(Card),
    );
  }

  testWidgets(
    'ホームから開くと受けたレッスンをサーバーの並びのまま出し、行ごとに日時・通話か動画添削か・コーチ・メニュー・評価したかを出す',
    (tester) async {
      final api = FakeStudentApi();
      final container = await openLessonCompletions(tester, api);

      expect(
        container.read(studentFlowProvider).step,
        FlowStep.lessonCompletions,
      );
      expect(api.calls.last, 'listMyLessonCompletions');
      final [async, reviewed, live] = Fixtures.lessonCompletions;
      expect(rows(tester), [
        [
          formatDateTime(async.completedAt),
          '非同期レッスン（動画添削）',
          '鈴木 コーチ',
          'メニュー: サーブの動画添削',
          'まだ評価していません',
          '評価する',
          '添削結果を見る',
        ],
        [
          formatDateTime(reviewed.completedAt),
          '同期レッスン（ビデオ通話）',
          '佐藤 コーチ',
          'メニュー: フォアハンドの基礎',
          '評価済み',
        ],
        // コーチの名前が引けなければ「担当コーチ」、メニューがなければ出さない
        [
          formatDateTime(live.completedAt),
          '同期レッスン（ビデオ通話）',
          '担当コーチ',
          'まだ評価していません',
          '評価する',
        ],
      ]);
    },
  );

  testWidgets('読み込みの間は読み込み中の旨を出す', (tester) async {
    final gate = Completer<void>();
    final api = FakeStudentApi()..lessonCompletionsGate = gate;
    await openLessonCompletions(tester, api, settle: false);

    expect(find.text('受けたレッスンを読み込んでいます…'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    expect(find.byType(Card), findsNothing);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('受けたレッスンを読み込んでいます…'), findsNothing);
    expect(rows(tester), hasLength(3));
  });

  testWidgets('受けたレッスンがなければ 0 件の旨を出す', (tester) async {
    final api = FakeStudentApi(lessonCompletions: const []);
    await openLessonCompletions(tester, api);

    expect(
      find.text('まだ受けたレッスンはありません。レッスンの完了を記録すると、ここで振り返れます。'),
      findsOneWidget,
    );
    expect(find.byType(Card), findsNothing);
  });

  testWidgets('読み込めなければその旨と読み直しのボタンを出し、読み直せたら一覧を出す', (tester) async {
    final failures = {
      'listMyLessonCompletions': const StudentApiException(statusCode: 500),
    };
    final api = FakeStudentApi(failures: failures);
    final container = await openLessonCompletions(tester, api);

    expect(find.text('受けたレッスンを読み込めませんでした。'), findsOneWidget);
    expect(find.text('評価する'), findsNothing);
    // 画面の欄にだけ旨を出し、直前の操作の失敗の文言にはしない
    expect(container.read(studentFlowProvider).errorMessage, isNull);
    expect(container.read(lessonCompletionsProvider).failed, isTrue);

    failures.clear();
    await tapText(tester, 'もう一度読み込む');

    expect(find.text('受けたレッスンを読み込めませんでした。'), findsNothing);
    expect(rows(tester), hasLength(3));
  });

  testWidgets('まだ評価していない行の「評価する」でそのレッスンを評価でき、戻ると評価済みになる', (tester) async {
    final api = FakeStudentApi();
    final container = await openLessonCompletions(tester, api);
    api.calls.clear();

    await tester.tap(
      find.descendant(of: rowOf('lesson-live'), matching: find.text('評価する')),
    );
    await tester.pumpAndSettle();

    expect(
      container.read(studentFlowProvider).step,
      FlowStep.lessonCompletionReview,
    );
    expect(find.text('同期レッスン（ビデオ通話）'), findsOneWidget);
    expect(find.text('担当コーチ'), findsOneWidget);
    // 受けたレッスンから来たときは一周シナリオの月額契約へは進ませない
    expect(find.text('月額契約へ進む'), findsNothing);

    await tapText(tester, ratingChoiceLabel(5));
    await tester.enterText(find.byType(TextField), 'わかりやすかった');
    await tester.pump();
    await tapText(tester, '評価を送る');

    expect(api.calls, ['submitLessonReview']);
    expect((api.requests.last as SubmitLessonReviewRequest).toJson(), {
      'lesson_id': 'lesson-live',
      'rating': 5,
      'comment': 'わかりやすかった',
    });
    expect(find.text('評価を送りました'), findsOneWidget);

    // サーバーも評価済みを返すようになる
    api.lessonCompletions[2] = Fixtures.lessonCompletion(
      lessonId: 'lesson-live',
      lessonType: LessonType.liveLesson,
      completedAgo: const Duration(days: 3),
      coachDisplayName: null,
      isReviewed: true,
    );
    await tester.tap(find.byTooltip('受けたレッスンに戻る'));
    await tester.pumpAndSettle();

    expect(
      container.read(studentFlowProvider).step,
      FlowStep.lessonCompletions,
    );
    expect(api.calls.last, 'listMyLessonCompletions');
    expect(
      find.descendant(of: rowOf('lesson-live'), matching: find.text('評価済み')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: rowOf('lesson-live'), matching: find.text('評価する')),
      findsNothing,
    );
  });

  testWidgets('動画添削の行の「添削結果を見る」でその動画の添削結果を開き、閉じると一覧に戻る', (tester) async {
    final api = FakeStudentApi(
      videoReviews: {'video-returned': Fixtures.reviewReturned},
    );
    final container = await openLessonCompletions(tester, api);
    api.calls.clear();
    api.requests.clear();

    await tapText(tester, '添削結果を見る');

    expect(container.read(studentFlowProvider).step, FlowStep.myVideoReview);
    expect(api.calls, ['getVideoReview']);
    expect(api.requests, ['video-returned']);
    expect(find.text('コーチの添削'), findsOneWidget);
    // 読むだけで、完了の記録には進ませない
    expect(find.text('完了の記録へ進む'), findsNothing);

    await tester.tap(find.byTooltip('添削結果を閉じる'));
    await tester.pumpAndSettle();

    expect(
      container.read(studentFlowProvider).step,
      FlowStep.lessonCompletions,
    );
    expect(rows(tester), hasLength(3));
  });

  testWidgets('添削結果を読めなければ一覧に留まって失敗の文言を出す', (tester) async {
    final api = FakeStudentApi()..videoReviewFailures.add('video-returned');
    final container = await openLessonCompletions(tester, api);

    await tapText(tester, '添削結果を見る');

    expect(
      container.read(studentFlowProvider).step,
      FlowStep.lessonCompletions,
    );
    expect(container.read(studentFlowProvider).errorMessage, isNotNull);
    expect(find.text('添削結果を見る'), findsOneWidget);
  });

  testWidgets('アプリバーの「ホーム」でホームに戻る', (tester) async {
    final api = FakeStudentApi();
    final container = await openLessonCompletions(tester, api);

    await tester.tap(find.byTooltip('ホーム'));
    await tester.pumpAndSettle();

    expect(container.read(studentFlowProvider).step, FlowStep.home);
  });
}
