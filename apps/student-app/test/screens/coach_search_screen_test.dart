import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/coach_profile.dart';
import 'package:rally_student_app/state/coach_search.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final home = StudentFlowState(
    step: FlowStep.home,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  /// ホームを描き、「コーチを探す」で開く（応答を待つかは [settle]）
  Future<ProviderContainer> openCoachSearch(
    WidgetTester tester,
    FakeStudentApi api, {
    bool settle = true,
  }) async {
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
    );
    final button = find.text('コーチを探す');
    await tester.ensureVisible(button);
    await tester.tap(button);
    if (settle) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
    }
    return container;
  }

  /// 一覧の行の表示名を、上から順に
  List<String> rowNames(WidgetTester tester) => [
    for (final tile in tester.widgetList<ListTile>(
      find.descendant(of: find.byType(Card), matching: find.byType(ListTile)),
    ))
      (tile.title! as Text).data!,
  ];

  testWidgets('ホームから開くと生徒のスポーツのコーチをサーバーの並びのまま出し、行ごとに評価の平均と件数・メニューの数を出す', (
    tester,
  ) async {
    final api = FakeStudentApi();
    final container = await openCoachSearch(tester, api);

    expect(container.read(studentFlowProvider).step, FlowStep.coachSearch);
    // スポーツは省き、サーバーが生徒自身のスポーツで探す
    expect(api.calls.last, 'listCoachesForStudent');
    expect(api.requests.whereType<Sport>(), isEmpty);
    expect(find.text('テニスのコーチ'), findsOneWidget);
    expect(rowNames(tester), ['佐藤', '鈴木', '高橋']);
    expect(find.text('★ 4.3（12 件）・レッスンメニュー 2 件'), findsOneWidget);
    expect(find.text('★ 3.7（3 件）・レッスンメニュー 1 件'), findsOneWidget);
    // 評価が 0 件のコーチは「評価なし」
    expect(find.text('評価なし・レッスンメニュー 0 件'), findsOneWidget);
  });

  testWidgets('お気に入りのコーチの行にだけハートのしるしを付ける', (tester) async {
    final api = FakeStudentApi(favoriteCoaches: [Fixtures.favoriteCoach()]);
    await openCoachSearch(tester, api);

    expect(find.byIcon(Icons.favorite), findsOneWidget);
    expect(
      find.descendant(
        of: find.ancestor(of: find.text('鈴木'), matching: find.byType(ListTile)),
        matching: find.byIcon(Icons.favorite),
      ),
      findsOneWidget,
    );
  });

  testWidgets('読み込みの間は読み込み中の旨を出す', (tester) async {
    final gate = Completer<void>();
    final api = FakeStudentApi()..coachListingsGate = gate;
    await openCoachSearch(tester, api, settle: false);

    expect(find.text('コーチを読み込んでいます…'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    expect(find.byType(ListTile), findsNothing);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('コーチを読み込んでいます…'), findsNothing);
    expect(rowNames(tester), ['佐藤', '鈴木', '高橋']);
  });

  testWidgets('該当するコーチがいなければ 0 件の旨を出す', (tester) async {
    final api = FakeStudentApi(coachListings: const []);
    await openCoachSearch(tester, api);

    expect(find.text('まだこのスポーツのコーチはいません。'), findsOneWidget);
    expect(find.byType(ListTile), findsNothing);
  });

  testWidgets('読み込めなければその旨と読み直しのボタンを出し、読み直せたら一覧を出す', (tester) async {
    final failures = {
      'listCoachesForStudent': const StudentApiException(statusCode: 500),
    };
    final api = FakeStudentApi(failures: failures);
    final container = await openCoachSearch(tester, api);

    expect(find.text('コーチの一覧を読み込めませんでした。'), findsOneWidget);
    expect(find.byType(ListTile), findsNothing);
    // 画面の欄にだけ旨を出し、直前の操作の失敗の文言にはしない
    expect(container.read(studentFlowProvider).errorMessage, isNull);
    expect(container.read(coachSearchProvider).failed, isTrue);

    failures.clear();
    await tapText(tester, 'もう一度読み込む');

    expect(find.text('コーチの一覧を読み込めませんでした。'), findsNothing);
    expect(rowNames(tester), ['佐藤', '鈴木', '高橋']);
  });

  testWidgets('行を押すとそのコーチの紹介を開く', (tester) async {
    final api = FakeStudentApi();
    final container = await openCoachSearch(tester, api);
    api.calls.clear();
    api.requests.clear();

    await tapText(tester, '鈴木');

    expect(container.read(studentFlowProvider).step, FlowStep.coachProfile);
    expect(container.read(coachProfileProvider).coach?.displayName, '鈴木');
    expect(api.calls, contains('getCoachProfile'));
    expect(api.requests, contains(Fixtures.coachListings[1].coachId));
  });

  testWidgets('アプリバーの「ホーム」でホームに戻る', (tester) async {
    final api = FakeStudentApi();
    final container = await openCoachSearch(tester, api);

    await tester.tap(find.byTooltip('ホーム'));
    await tester.pumpAndSettle();

    expect(container.read(studentFlowProvider).step, FlowStep.home);
  });
}
