import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/video_submit_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  // 登録はテニスだが、サッカーのスロットを体験予約した生徒
  final soccerSlot = Slot(
    slotId: Fixtures.slotId,
    coachId: Fixtures.coachId,
    sport: Sport.soccer,
    startsAt: Fixtures.slot.startsAt,
    endsAt: Fixtures.slot.endsAt,
  );

  testWidgets('動画を選んで（モック）送付すると、体験予約のコーチとスロットのスポーツで送って添削結果に進む', (tester) async {
    final api = FakeStudentApi(reviews: [Fixtures.reviewInProgress]);
    final container = await pumpScreen(
      tester,
      const VideoSubmitScreen(),
      api: api,
      initial: StudentFlowState(
        step: FlowStep.videoSubmit,
        student: Fixtures.student,
        bookedSlot: soccerSlot,
        booking: Fixtures.bookingConfirmed,
      ),
    );

    final submit = find.widgetWithText(FilledButton, '送付する');
    expect(tester.widget<FilledButton>(submit).onPressed, isNull);

    await tapText(tester, '動画を選ぶ（モック）');
    expect(find.text('選んだ動画: swing.mp4'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'フォアハンドを見てください');
    await tapText(tester, '送付する');

    expect(api.calls, [
      'listCoachLessonMenus',
      'submitVideo',
      'getVideoReview',
    ]);
    final request = api.requests.whereType<SubmitVideoRequest>().single;
    expect(request.toJson(), {
      'coach_id': Fixtures.coachId,
      'sport': 'soccer',
      'content_type': 'video/mp4',
      'note': 'フォアハンドを見てください',
    });
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.videoReview);
    expect(state.review!.status, VideoReviewStatus.inProgress);
  });

  group('動画の送付のレッスンメニュー', () {
    /// 体験予約のコーチのテニスの非同期のメニュー（説明あり）
    final swingMenu = LessonMenu(
      lessonMenuId: '15151515-1515-4515-8515-151515151515',
      coachId: Fixtures.coachId,
      sport: Sport.tennis,
      lessonType: LessonType.asyncLesson,
      name: 'スイングの動画添削',
      description: 'フォームを動画で見直します',
      isPublished: true,
      createdAt: Fixtures.now,
    );

    FakeStudentApi apiWithMenus({Map<String, StudentApiException>? failures}) =>
        FakeStudentApi(
          reviews: [Fixtures.reviewInProgress],
          lessonMenus: {
            Fixtures.coachId: [...Fixtures.lessonMenus, swingMenu],
          },
          failures: failures ?? {},
        );

    SubmitVideoRequest lastRequest(FakeStudentApi api) =>
        api.requests.whereType<SubmitVideoRequest>().last;

    double topOf(WidgetTester tester, String text) =>
        tester.getTopLeft(find.text(text)).dy;

    Future<ProviderContainer> open(
      WidgetTester tester,
      FakeStudentApi api, {
      Slot? bookedSlot,
    }) async {
      await tester.binding.setSurfaceSize(const Size(800, 1600));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final container = await pumpScreen(
        tester,
        const VideoSubmitScreen(),
        api: api,
        initial: StudentFlowState(
          step: FlowStep.videoSubmit,
          student: Fixtures.student,
          bookedSlot: bookedSlot ?? Fixtures.slot,
          booking: Fixtures.bookingConfirmed,
        ),
      );
      await tapText(tester, '動画を選ぶ（モック）');
      return container;
    }

    testWidgets('体験予約のコーチの非同期のメニューを名前と説明で出し、既定は「指定しない」でメニューなしで送る', (
      tester,
    ) async {
      final api = apiWithMenus();
      final container = await open(tester, api);

      expect(api.requests.first, (
        coachId: Fixtures.coachId,
        sport: Sport.tennis,
      ));
      expect(find.text('指定しない'), findsOneWidget);
      expect(find.text('サーブの動画添削'), findsOneWidget);
      expect(find.text('スイングの動画添削'), findsOneWidget);
      expect(find.text('フォームを動画で見直します'), findsOneWidget);
      // 同期のメニューは出さない
      expect(find.text('フォアハンドの基礎'), findsNothing);
      // 作った時刻の古い順
      expect(topOf(tester, 'サーブの動画添削'), lessThan(topOf(tester, 'スイングの動画添削')));

      await tapText(tester, '送付する');

      expect(lastRequest(api).toJson(), isNot(contains('lesson_menu_id')));
      expect(container.read(studentFlowProvider).step, FlowStep.videoReview);
    });

    testWidgets('メニューを選んで送ると lesson_menu_id を付けて submitVideo する', (
      tester,
    ) async {
      final api = apiWithMenus();
      final container = await open(tester, api);

      await tapText(tester, 'スイングの動画添削');
      await tapText(tester, '送付する');

      expect(lastRequest(api).toJson(), {
        'coach_id': Fixtures.coachId,
        'sport': 'tennis',
        'content_type': 'video/mp4',
        'lesson_menu_id': swingMenu.lessonMenuId,
      });
      expect(container.read(studentFlowProvider).step, FlowStep.videoReview);
    });

    testWidgets('メニューがなければ選択の欄を出さない', (tester) async {
      final api = FakeStudentApi(reviews: [Fixtures.reviewInProgress]);
      await open(tester, api);

      expect(api.calls, ['listCoachLessonMenus']);
      expect(find.text('レッスンメニュー'), findsNothing);
      expect(find.text('指定しない'), findsNothing);
    });

    testWidgets('スロットのスポーツのメニューがなければ、生徒のスポーツのメニューがあっても選択の欄を出さない', (
      tester,
    ) async {
      final api = apiWithMenus();
      await open(tester, api, bookedSlot: soccerSlot);

      expect(find.text('指定しない'), findsNothing);
      expect(find.text('スイングの動画添削'), findsNothing);
    });

    testWidgets('メニューを付けて 422 で断られたら、メニューが変わった可能性を伝えて読み直し、動画と相談内容はそのまま選び直させる', (
      tester,
    ) async {
      final failures = <String, StudentApiException>{
        'submitVideo': Fixtures.validationFailed,
      };
      final api = apiWithMenus(failures: failures);
      final container = await open(tester, api);
      await tester.enterText(find.byType(TextField), 'サーブを見てください');
      await tapText(tester, 'スイングの動画添削');

      // 選んだ後にコーチがメニューを非公開にした
      api.lessonMenus[Fixtures.coachId]!.remove(swingMenu);
      await tapText(tester, '送付する');

      expect(api.calls, [
        'listCoachLessonMenus',
        'submitVideo',
        'listCoachLessonMenus',
      ]);
      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.videoSubmit);
      expect(state.errorMessage, isNull);
      expect(find.textContaining('メニューが非公開になったか'), findsOneWidget);
      expect(find.text('選んだ動画: swing.mp4'), findsOneWidget);
      expect(find.text('サーブを見てください'), findsOneWidget);
      expect(find.text('スイングの動画添削'), findsNothing);
      final submit = find.widgetWithText(FilledButton, '送付する');
      expect(tester.widget<FilledButton>(submit).onPressed, isNotNull);

      failures.clear();
      await tapText(tester, '送付する');

      expect(lastRequest(api).toJson(), {
        'coach_id': Fixtures.coachId,
        'sport': 'tennis',
        'content_type': 'video/mp4',
        'note': 'サーブを見てください',
      });
      expect(container.read(studentFlowProvider).step, FlowStep.videoReview);
    });

    testWidgets('メニューを付けずに断られたら、今までどおり失敗の文言を出す', (tester) async {
      final api = apiWithMenus(
        failures: {'submitVideo': Fixtures.validationFailed},
      );
      final container = await open(tester, api);
      await tapText(tester, '送付する');

      final state = container.read(studentFlowProvider);
      expect(state.step, FlowStep.videoSubmit);
      expect(state.errorMessage, '入力内容を確かめてください。');
      expect(find.textContaining('メニューが非公開になったか'), findsNothing);
      expect(api.calls, ['listCoachLessonMenus', 'submitVideo']);
    });

    testWidgets('メニューを読めなければその旨を出し、メニューなしで送れる', (tester) async {
      final api = apiWithMenus(
        failures: {'listCoachLessonMenus': Fixtures.unreachable},
      );
      final container = await open(tester, api);

      expect(
        find.text('レッスンメニューを読み込めませんでした。メニューを指定せずに依頼できます。'),
        findsOneWidget,
      );
      expect(find.text('指定しない'), findsNothing);
      await tapText(tester, '送付する');

      expect(lastRequest(api).toJson(), isNot(contains('lesson_menu_id')));
      expect(container.read(studentFlowProvider).step, FlowStep.videoReview);
    });
  });
}
