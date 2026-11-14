import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/ui/flow_scaffold.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  StudentFlowState at(FlowStep step, {bool signedIn = true}) =>
      StudentFlowState(
        step: step,
        accessToken: signedIn ? Fixtures.registered.accessToken : null,
        student: signedIn ? Fixtures.student : null,
      );

  Future<void> pumpAt(
    WidgetTester tester,
    StudentFlowState initial, {
    double width = 390,
    bool showNavigation = true,
  }) async {
    tester.view.physicalSize = Size(width, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await pumpScreen(
      tester,
      FlowScaffold(
        title: 'コーチを探す',
        showNavigation: showNavigation,
        children: const [],
      ),
      api: FakeStudentApi(),
      initial: initial,
    );
  }

  AppBar appBar(WidgetTester tester) =>
      tester.widget<AppBar>(find.byType(AppBar));

  for (final width in [360.0, 390.0, 402.0, 430.0]) {
    testWidgets('幅 $width: ホーム以外ではタイトルをアイコンの行の下に切れずに出す', (tester) async {
      await pumpAt(tester, at(FlowStep.coachSearch), width: width);

      expect(appBar(tester).title, isNull);
      expect(appBar(tester).bottom, isNotNull);
      final title = find.descendant(
        of: find.byType(AppBar),
        matching: find.text('コーチを探す'),
      );
      expect(title, findsOneWidget);
      expect(
        tester.renderObject<RenderParagraph>(title).didExceedMaxLines,
        isFalse,
      );
      expect(tester.getSize(title).width, lessThanOrEqualTo(width - 32));
      expect(find.byTooltip('ホーム'), findsOneWidget);
    });
  }

  testWidgets('ホームでは今までどおりアプリバーの title に出し、ホームの入口は出さない', (tester) async {
    await pumpAt(tester, at(FlowStep.home));

    expect(appBar(tester).title, isNotNull);
    expect(appBar(tester).bottom, isNull);
    expect(find.byTooltip('ホーム'), findsNothing);
    expect(find.byTooltip('自分の予約'), findsOneWidget);
  });

  testWidgets('入口を出さない画面（showNavigation: false・未登録）では title に出す', (
    tester,
  ) async {
    await pumpAt(tester, at(FlowStep.coachSearch), showNavigation: false);
    expect(appBar(tester).title, isNotNull);
    expect(appBar(tester).bottom, isNull);

    await pumpAt(tester, at(FlowStep.coachSearch, signedIn: false));
    expect(appBar(tester).title, isNotNull);
    expect(appBar(tester).bottom, isNull);
  });

  testWidgets('脇の画面（自分の予約など）は入口を出さないので title に出す', (tester) async {
    await pumpAt(tester, at(FlowStep.myBookings));
    expect(appBar(tester).title, isNotNull);
    expect(appBar(tester).bottom, isNull);
  });
}
