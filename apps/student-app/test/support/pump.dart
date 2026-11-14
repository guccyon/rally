import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/server_clock.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/coach_profile.dart';
import 'package:rally_student_app/state/coach_search.dart';
import 'package:rally_student_app/state/favorite_coaches.dart';
import 'package:rally_student_app/state/lesson_completions.dart';
import 'package:rally_student_app/state/lesson_review.dart';
import 'package:rally_student_app/device/local_media.dart';
import 'package:rally_student_app/state/notifications.dart';
import 'package:rally_student_app/state/prejoin_check.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';

import 'fake_local_media.dart';
import 'fake_student_api.dart';

/// [screen] を、API をモックにした ProviderScope で描く。[initial] で途中の段から始め、[chat] でチャットの状態、[review] で完了したレッスンの評価の状態、[subscription] で月額契約と受講権の状態、[coachSearch] でコーチを探す画面の状態、[coachProfile] でコーチの紹介の状態、[favoriteCoaches] でお気に入りのコーチの状態、[notifications] でお知らせの状態、[lessonCompletions] で受けたレッスンの状態、[prejoin] で入室前チェックの状態を渡す。
/// 端末のカメラ・マイク・スピーカーは [media]（既定はどれも使える作り物の端末）。
/// 今の時刻は [now]（既定は [Fixtures.now]）に固定する。時刻を進めるテストは [clock] を渡す。
/// 端末の時計とサーバーの時計のずれを確かめるテストは [serverClock]（端末の時計を差し替えたもの）を渡す。そのときは今の時刻を固定せず、
/// サーバーに合わせた時計（`listMyBookings` などの応答のずれ。[FakeStudentApi.clockOffset]）で数える。
Future<ProviderContainer> pumpScreen(
  WidgetTester tester,
  Widget screen, {
  required FakeStudentApi api,
  StudentFlowState? initial,
  ChatState? chat,
  LessonReviewState? review,
  SubscriptionState? subscription,
  CoachSearchState? coachSearch,
  CoachProfileState? coachProfile,
  FavoriteCoachesState? favoriteCoaches,
  NotificationsState? notifications,
  LessonCompletionsState? lessonCompletions,
  PrejoinCheckState? prejoin,
  FakeLocalMedia? media,
  DateTime? now,
  DateTime Function()? clock,
  ServerClock? serverClock,
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
        coachProfileProvider.overrideWith(
          () => CoachProfileNotifier(coachProfile),
        ),
        favoriteCoachesProvider.overrideWith(
          () => FavoriteCoachesNotifier(favoriteCoaches),
        ),
        notificationsProvider.overrideWith(
          () => NotificationsNotifier(notifications),
        ),
        lessonCompletionsProvider.overrideWith(
          () => LessonCompletionsNotifier(lessonCompletions),
        ),
        prejoinCheckProvider.overrideWith(() => PrejoinCheckNotifier(prejoin)),
        localMediaProvider.overrideWithValue(media ?? FakeLocalMedia()),
        if (serverClock != null)
          serverClockProvider.overrideWith(
            () => ServerClockNotifier(serverClock),
          )
        else
          clockProvider.overrideWithValue(clock ?? () => now ?? Fixtures.now),
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

/// [finder] のボタンを、次のフレームを待たずにすばやく 2 回押して、落ち着くまで進める。
/// Web 版はセマンティクスの木（画面の読み上げや自動操作が押す要素）からの押下を、ポインターの吸収（Navigator が閉じた直後の押下を捨てる）を通さずに
/// ボタンへ渡すので、閉じ始めたダイアログのボタンが 2 回押される（#1513）。ここでもセマンティクスの tap で押す。
Future<void> doubleTapInOneFrame(WidgetTester tester, Finder finder) async {
  final semantics = tester.ensureSemantics();
  final node = tester.getSemantics(finder);
  node.owner!.performAction(node.id, SemanticsAction.tap);
  node.owner!.performAction(node.id, SemanticsAction.tap);
  await tester.pumpAndSettle();
  semantics.dispose();
}
