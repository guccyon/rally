import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/coach_search.dart';
import 'package:rally_student_app/state/lesson_review.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';

import 'fake_student_api.dart';

/// [screen] を、API をモックにした ProviderScope で描く。[initial] で途中の段から始め、[chat] でチャットの状態、[review] で完了したレッスンの評価の状態、[subscription] で月額契約と受講権の状態、[coachSearch] でコーチを探す画面の状態を渡す。
/// 今の時刻は [now]（既定は [Fixtures.now]）に固定する。
Future<ProviderContainer> pumpScreen(
  WidgetTester tester,
  Widget screen, {
  required FakeStudentApi api,
  StudentFlowState? initial,
  ChatState? chat,
  LessonReviewState? review,
  SubscriptionState? subscription,
  CoachSearchState? coachSearch,
  DateTime? now,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        studentFlowProvider.overrideWith(
          () => StudentFlowNotifier(initial ?? StudentFlowState()),
        ),
        chatProvider.overrideWith(() => ChatNotifier(chat)),
        lessonReviewProvider.overrideWith(() => LessonReviewNotifier(review)),
        subscriptionProvider.overrideWith(
          () => SubscriptionNotifier(subscription),
        ),
        coachSearchProvider.overrideWith(
          () => CoachSearchNotifier(coachSearch),
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
