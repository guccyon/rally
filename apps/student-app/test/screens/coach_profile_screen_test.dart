import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/coach_profile.dart';
import 'package:rally_student_app/state/favorite_coaches.dart';
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

  /// コーチを探す一覧の先頭（佐藤。[Fixtures.slot] のコーチ）
  final sato = Fixtures.coachListings.first;

  /// ホームから「コーチを探す」を開き、[name] の行を押してコーチの紹介を開く
  Future<ProviderContainer> openCoachProfile(
    WidgetTester tester,
    FakeStudentApi api, {
    String name = '佐藤',
  }) async {
    // 紹介の欄をすべて一度に描く（ListView は画面の外の行を作らない）
    await tester.binding.setSurfaceSize(const Size(800, 2000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
    );
    await tapText(tester, 'コーチを探す');
    await tapText(tester, name);
    return container;
  }

  /// 評価が 0 件のコーチ
  CoachReviewSummary noReviews(String coachId) =>
      CoachReviewSummary(coachId: coachId, reviewCount: 0, averageRating: null);

  testWidgets('一覧の行から開くと、表示名・自己紹介・評価の平均と件数・最近の評価・公開中のレッスンメニューがそろう', (
    tester,
  ) async {
    final api = FakeStudentApi(
      lessonMenus: {Fixtures.coachId: Fixtures.lessonMenus},
    );
    final container = await openCoachProfile(tester, api);

    expect(container.read(studentFlowProvider).step, FlowStep.coachProfile);
    expect(
      api.calls,
      containsAll([
        'getCoachProfile',
        'getCoachReviewSummary',
        'listCoachReviews',
        'listCoachLessonMenus',
      ]),
    );
    expect(api.requests, contains(Fixtures.coachId));
    // メニューはコーチを探したスポーツ（生徒のスポーツ）で絞る
    expect(
      api.requests,
      contains((coachId: Fixtures.coachId, sport: Sport.tennis)),
    );

    expect(find.text('佐藤'), findsOneWidget);
    expect(find.text(Fixtures.coachBio), findsOneWidget);
    expect(find.text('★ 4.5（2 件）'), findsOneWidget);
    expect(find.text('最近の評価'), findsOneWidget);
    expect(find.text('5 点（5 点満点）'), findsOneWidget);
    expect(find.text('メニュー: フォアハンドの基礎'), findsOneWidget);
    expect(find.text('丁寧でした'), findsOneWidget);
    expect(find.text('わかりやすかった'), findsOneWidget);
    expect(find.text('フォアハンドの基礎'), findsOneWidget);
    expect(find.text('テニス・同期レッスン（ビデオ通話）'), findsOneWidget);
    expect(find.text('1 回 60 分'), findsOneWidget);
    expect(find.text('サーブの動画添削'), findsOneWidget);
    // ほかのスポーツのメニューは出さない
    expect(find.text('バッティングの基礎'), findsNothing);
    expect(find.textContaining('読み込めませんでした'), findsNothing);
    expect(find.text('このコーチの空き枠を見る'), findsOneWidget);
  });

  testWidgets('最近の評価は新しい順に 3 件まで出す', (tester) async {
    final reviews = [
      for (var i = 0; i < 4; i++)
        CoachReview(
          reviewId: 'review-$i',
          lessonId: 'lesson-$i',
          rating: 5 - i,
          comment: '一言 $i',
          submittedAt: Fixtures.now.subtract(Duration(days: i)),
        ),
    ];
    final api = FakeStudentApi(coachReviews: reviews);
    await openCoachProfile(tester, api);

    expect(find.text('一言 0'), findsOneWidget);
    expect(find.text('一言 1'), findsOneWidget);
    expect(find.text('一言 2'), findsOneWidget);
    expect(find.text('一言 3'), findsNothing);
  });

  testWidgets('最近の評価のうち、コーチの返信がある評価だけ、その下に「コーチからの返信」と返信の文と時刻を出す', (
    tester,
  ) async {
    await openCoachProfile(tester, FakeStudentApi());

    final [newer, older] = Fixtures.coachReviews;
    expect(older.reply, isNull);
    final reply = newer.reply!;
    expect(find.text('コーチからの返信'), findsOneWidget, reason: '返信のない評価には出さない');
    expect(find.text(reply.comment), findsOneWidget);
    expect(find.text(formatDateTime(reply.repliedAt)), findsOneWidget);
    final replyTop = tester.getTopLeft(find.text('コーチからの返信')).dy;
    // 返信は返信先の評価（一言）の下、次の評価の上
    expect(
      replyTop,
      greaterThan(tester.getTopLeft(find.text(newer.comment)).dy),
    );
    expect(
      tester.getTopLeft(find.text(reply.comment)).dy,
      greaterThan(replyTop),
    );
    expect(
      tester.getTopLeft(find.text(older.comment)).dy,
      greaterThan(tester.getTopLeft(find.text(reply.comment)).dy),
    );
  });

  testWidgets('返信のある評価がなければ「コーチからの返信」は出さない', (tester) async {
    final api = FakeStudentApi(
      coachReviews: [
        for (final review in Fixtures.coachReviews)
          CoachReview(
            reviewId: review.reviewId,
            lessonId: review.lessonId,
            rating: review.rating,
            comment: review.comment,
            submittedAt: review.submittedAt,
            lessonMenu: review.lessonMenu,
          ),
      ],
    );
    await openCoachProfile(tester, api);

    expect(find.text('丁寧でした'), findsOneWidget);
    expect(find.text('コーチからの返信'), findsNothing);
  });

  testWidgets('最近の評価を読み込んでいる間はその旨を出し、返信はまだ出さない', (tester) async {
    final gate = Completer<void>();
    final api = FakeStudentApi()..coachReviewsGate = gate;
    await tester.binding.setSurfaceSize(const Size(800, 2000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: home,
    );
    await tapText(tester, 'コーチを探す');
    await tester.tap(find.text('佐藤'));
    await tester.pump();
    await tester.pump();

    expect(find.text('最近の評価を読み込んでいます…'), findsOneWidget);
    expect(find.text('コーチからの返信'), findsNothing);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('最近の評価を読み込んでいます…'), findsNothing);
    expect(find.text('コーチからの返信'), findsOneWidget);
  });

  testWidgets('自己紹介を書いていないコーチは、自己紹介がまだない旨を出す', (tester) async {
    final api = FakeStudentApi(
      coachProfiles: {
        sato.coachId: CoachProfile(
          coachId: sato.coachId,
          displayName: sato.displayName,
          bio: null,
          sports: const [Sport.tennis],
        ),
      },
    );
    await openCoachProfile(tester, api);

    expect(find.text('まだ自己紹介はありません。'), findsOneWidget);
    expect(find.text(Fixtures.coachBio), findsNothing);
    // ほかの欄はそのまま
    expect(find.text('★ 4.5（2 件）'), findsOneWidget);
  });

  testWidgets('評価が 0 件のコーチは「評価はまだありません」で、最近の評価の欄を出さない', (tester) async {
    final takahashi = Fixtures.coachListings.last;
    final api = FakeStudentApi(
      coachReviewSummaries: {takahashi.coachId: noReviews(takahashi.coachId)},
      coachReviews: const [],
    );
    await openCoachProfile(tester, api, name: '高橋');

    expect(find.text('高橋'), findsOneWidget);
    expect(find.text('評価はまだありません'), findsOneWidget);
    expect(find.text('最近の評価'), findsNothing);
    expect(find.text('コーチからの返信'), findsNothing);
    expect(find.text('まだ公開中のレッスンメニューはありません。'), findsOneWidget);
  });

  testWidgets('自己紹介を読めなくても、ほかの欄と空き枠への移動はそのままで、その欄だけ読み直せる', (tester) async {
    final failures = {
      'getCoachProfile': const StudentApiException(statusCode: 500),
    };
    final api = FakeStudentApi(
      failures: failures,
      lessonMenus: {Fixtures.coachId: Fixtures.lessonMenus},
    );
    final container = await openCoachProfile(tester, api);

    expect(find.text('自己紹介を読み込めませんでした。'), findsOneWidget);
    // 表示名は一覧で選んだコーチのものを出す
    expect(find.text('佐藤'), findsOneWidget);
    expect(find.text('★ 4.5（2 件）'), findsOneWidget);
    expect(find.text('丁寧でした'), findsOneWidget);
    expect(find.text('フォアハンドの基礎'), findsOneWidget);
    // 欄にだけ旨を出し、直前の操作の失敗の文言にはしない
    expect(container.read(studentFlowProvider).errorMessage, isNull);
    expect(
      container.read(coachProfileProvider).failures,
      {CoachProfileSection.profile},
    );

    failures.clear();
    api.calls.clear();
    await tapText(tester, 'もう一度読み込む');

    // 読み直すのは失敗した欄だけ
    expect(api.calls, ['getCoachProfile']);
    expect(find.text('自己紹介を読み込めませんでした。'), findsNothing);
    expect(find.text(Fixtures.coachBio), findsOneWidget);
  });

  testWidgets('評価・最近の評価・メニューを読めなければ、その欄ごとに旨を出し、自己紹介はそのまま出す', (
    tester,
  ) async {
    const error = StudentApiException(statusCode: 500);
    final api = FakeStudentApi(
      failures: {
        'getCoachReviewSummary': error,
        'listCoachReviews': error,
        'listCoachLessonMenus': error,
      },
    );
    final container = await openCoachProfile(tester, api);

    expect(find.text(Fixtures.coachBio), findsOneWidget);
    expect(find.text('評価の平均と件数を読み込めませんでした。'), findsOneWidget);
    expect(find.text('最近の評価を読み込めませんでした。'), findsOneWidget);
    expect(find.text('コーチからの返信'), findsNothing);
    expect(find.text('レッスンメニューを読み込めませんでした。'), findsOneWidget);
    expect(find.text('もう一度読み込む'), findsNWidgets(3));
    expect(container.read(studentFlowProvider).errorMessage, isNull);

    // 空き枠への移動は使える
    await tapText(tester, 'このコーチの空き枠を見る');
    expect(container.read(studentFlowProvider).step, FlowStep.slotSearch);
  });

  testWidgets('「このコーチの空き枠を見る」でそのコーチに絞った予約枠の検索へ移り、見出しの「絞り込みを外す」で外せる', (
    tester,
  ) async {
    final api = FakeStudentApi();
    final container = await openCoachProfile(tester, api);
    api.calls.clear();

    await tapText(tester, 'このコーチの空き枠を見る');

    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.slotSearch);
    expect(state.searchCoach?.coachId, sato.coachId);
    expect(api.calls.first, 'searchSlots');
    expect(api.slotSearchCoachIds, [sato.coachId]);
    expect(api.requests.whereType<Sport>().last, Sport.tennis);
    expect(find.text('佐藤 コーチの空き枠'), findsOneWidget);
    expect(find.text('絞り込みを外す'), findsOneWidget);
    expect(find.text('体験予約する'), findsOneWidget);

    await tapText(tester, '絞り込みを外す');

    expect(container.read(studentFlowProvider).searchCoach, isNull);
    expect(api.calls.where((c) => c == 'searchSlots'), hasLength(2));
    // 2 回目はコーチで絞らない
    expect(api.slotSearchCoachIds, [sato.coachId]);
    expect(find.text('佐藤 コーチの空き枠'), findsNothing);
    expect(find.text('絞り込みを外す'), findsNothing);
  });

  testWidgets('コーチで絞った検索から、今までどおり体験予約して確定待ちに進む', (tester) async {
    final api = FakeStudentApi();
    final container = await openCoachProfile(tester, api);
    await tapText(tester, 'このコーチの空き枠を見る');

    await tapText(tester, '体験予約する');

    final request = api.requests.last as CreateBookingRequest;
    expect(request.toJson(), {'slot_id': Fixtures.slotId, 'is_trial': true});
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.bookingPending);
    expect(state.bookedSlot?.coachId, sato.coachId);
  });

  testWidgets('コーチで絞った検索の読み込みの間は、読み込み中の旨を出し、前の検索結果を出さない', (tester) async {
    final api = FakeStudentApi();
    final container = await openCoachProfile(tester, api);
    // 先に絞らずに探した結果がある状態にしておく
    await container
        .read(studentFlowProvider.notifier)
        .searchSlots(Sport.tennis);
    final gate = Completer<void>();
    api.slotsGate = gate;

    await tester.tap(find.text('このコーチの空き枠を見る'));
    await tester.pump();

    expect(find.text('佐藤 コーチの空き枠'), findsOneWidget);
    expect(find.text('予約できるスロットを読み込んでいます…'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    expect(find.text('体験予約する'), findsNothing);

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('予約できるスロットを読み込んでいます…'), findsNothing);
    expect(find.text('体験予約する'), findsOneWidget);
  });

  testWidgets('そのコーチの空き枠がなければ、その旨を出し、絞り込みを外せる', (tester) async {
    final api = FakeStudentApi(slots: const []);
    await openCoachProfile(tester, api);

    await tapText(tester, 'このコーチの空き枠を見る');

    expect(find.text('佐藤 コーチの空き枠'), findsOneWidget);
    expect(find.text('このコーチの、このスポーツの予約できる空き枠はありません。'), findsOneWidget);
    expect(find.text('体験予約する'), findsNothing);

    await tapText(tester, '絞り込みを外す');
    expect(find.text('予約できるスロットはありません。'), findsOneWidget);
  });

  testWidgets('そのコーチの空き枠を読めなければ、その旨と読み直しのボタンを出し、同じ絞り込みで読み直せる', (tester) async {
    final failures = {'searchSlots': Fixtures.unreachable};
    final api = FakeStudentApi(failures: failures);
    final container = await openCoachProfile(tester, api);

    await tapText(tester, 'このコーチの空き枠を見る');

    expect(find.text('佐藤 コーチの空き枠'), findsOneWidget);
    expect(find.text('予約できるスロットを読み込めませんでした。'), findsOneWidget);
    expect(find.text('体験予約する'), findsNothing);
    // 失敗の文言は出さず、検索の欄にだけ出す
    expect(container.read(studentFlowProvider).errorMessage, isNull);

    failures.clear();
    await tapText(tester, 'もう一度読み込む');

    expect(api.slotSearchCoachIds, [sato.coachId, sato.coachId]);
    expect(api.requests.whereType<Sport>().last, Sport.tennis);
    expect(find.text('予約できるスロットを読み込めませんでした。'), findsNothing);
    expect(find.text('体験予約する'), findsOneWidget);
    expect(container.read(studentFlowProvider).slotsFailed, isFalse);
  });

  testWidgets('ホームの「スロットを探して予約する」ではコーチで絞らない', (tester) async {
    final api = FakeStudentApi();
    final container = await openCoachProfile(tester, api);
    await tapText(tester, 'このコーチの空き枠を見る');

    await tester.tap(find.byTooltip('ホーム'));
    await tester.pumpAndSettle();
    await tapText(tester, 'スロットを探して予約する');

    expect(container.read(studentFlowProvider).searchCoach, isNull);
    expect(api.slotSearchCoachIds, [sato.coachId]);
  });

  testWidgets('アプリバーの戻るでコーチの一覧に戻る（一覧は読み直さない）', (tester) async {
    final api = FakeStudentApi();
    final container = await openCoachProfile(tester, api);
    api.calls.clear();

    await tester.tap(find.byTooltip('コーチの一覧に戻る'));
    await tester.pumpAndSettle();

    expect(container.read(studentFlowProvider).step, FlowStep.coachSearch);
    expect(api.calls, isEmpty);
    expect(find.text('鈴木'), findsOneWidget);
  });

  testWidgets('「お気に入りに入れる」で入れ、一覧に戻ると読み直さずにしるしが付き、「お気に入りから外す」で外せる', (
    tester,
  ) async {
    final api = FakeStudentApi();
    final container = await openCoachProfile(tester, api);
    expect(find.byIcon(Icons.favorite_border), findsOneWidget);
    api.calls.clear();
    api.requests.clear();

    await tapText(tester, 'お気に入りに入れる');

    expect(api.calls, ['addFavoriteCoach']);
    expect(api.requests, [sato.coachId]);
    expect(find.text('お気に入りから外す'), findsOneWidget);
    expect(find.byIcon(Icons.favorite), findsOneWidget);
    expect(container.read(coachProfileProvider).profile?.isFavorite, isTrue);
    // ホームの欄にも応答の一覧を写す
    expect(
      container
          .read(favoriteCoachesProvider)
          .favoriteCoaches
          ?.map((c) => c.coachId),
      [sato.coachId],
    );

    await tester.tap(find.byTooltip('コーチの一覧に戻る'));
    await tester.pumpAndSettle();
    expect(api.calls, ['addFavoriteCoach']);
    expect(find.byIcon(Icons.favorite), findsOneWidget);
    expect(
      find.descendant(
        of: find.ancestor(of: find.text('佐藤'), matching: find.byType(ListTile)),
        matching: find.byIcon(Icons.favorite),
      ),
      findsOneWidget,
    );

    await tapText(tester, '佐藤');
    // 開き直すと紹介を読み直し、お気に入りのまま
    expect(find.text('お気に入りから外す'), findsOneWidget);
    api.calls.clear();
    await tapText(tester, 'お気に入りから外す');

    expect(api.calls, ['removeFavoriteCoach']);
    expect(find.text('お気に入りに入れる'), findsOneWidget);
    expect(container.read(favoriteCoachesProvider).favoriteCoaches, isEmpty);
    await tester.tap(find.byTooltip('コーチの一覧に戻る'));
    await tester.pumpAndSettle();
    expect(find.byIcon(Icons.favorite), findsNothing);
  });

  testWidgets('お気に入りに入っているコーチは「お気に入りから外す」で開く', (tester) async {
    final api = FakeStudentApi(
      favoriteCoaches: [Fixtures.favoriteCoach(coachId: sato.coachId)],
    );
    await openCoachProfile(tester, api);

    expect(find.text('お気に入りから外す'), findsOneWidget);
    expect(find.text('お気に入りに入れる'), findsNothing);
  });

  testWidgets('お気に入りを切り替えられなければ失敗の文言を出し、切り替える前のまま', (tester) async {
    final api = FakeStudentApi(
      failures: {'addFavoriteCoach': Fixtures.unreachable},
    );
    final container = await openCoachProfile(tester, api);

    await tapText(tester, 'お気に入りに入れる');

    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    expect(find.text('お気に入りに入れる'), findsOneWidget);
    expect(container.read(coachProfileProvider).profile?.isFavorite, isFalse);
    expect(container.read(favoriteCoachesProvider).favoriteCoaches, isNull);
  });

  testWidgets('紹介を読めなければ、お気に入りの切り替えは押せない', (tester) async {
    final api = FakeStudentApi(
      failures: {'getCoachProfile': Fixtures.unreachable},
    );
    await openCoachProfile(tester, api);

    final button = tester.widget<OutlinedButton>(
      find.ancestor(
        of: find.text('お気に入りに入れる'),
        matching: find.byWidgetPredicate((w) => w is OutlinedButton),
      ),
    );
    expect(button.onPressed, isNull);
    expect(find.text('自己紹介を読み込めませんでした。'), findsOneWidget);
  });
}
