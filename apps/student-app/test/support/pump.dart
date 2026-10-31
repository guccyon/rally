import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/state/student_flow.dart';

import 'fake_student_api.dart';

/// [screen] を、API をモックにした ProviderScope で描く。[initial] で途中の段から始める。
/// 今の時刻は [now]（既定は [Fixtures.now]）に固定する。
Future<ProviderContainer> pumpScreen(
  WidgetTester tester,
  Widget screen, {
  required FakeStudentApi api,
  StudentFlowState? initial,
  DateTime? now,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        studentFlowProvider.overrideWith(
          () => StudentFlowNotifier(initial ?? StudentFlowState()),
        ),
        clockProvider.overrideWithValue(() => now ?? Fixtures.now),
      ],
      child: MaterialApp(home: screen),
    ),
  );
  return ProviderScope.containerOf(tester.element(find.byWidget(screen)));
}

/// 文言で探したボタンなどを、画面内に出してから押す。
Future<void> tapText(WidgetTester tester, String text) async {
  final finder = find.text(text);
  await tester.ensureVisible(finder);
  await tester.tap(finder);
  await tester.pumpAndSettle();
}
