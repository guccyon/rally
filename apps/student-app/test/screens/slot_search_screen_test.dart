import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/slot_search_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/ui/labels.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final registered = StudentFlowState(
    step: FlowStep.slotSearch,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  testWidgets('スポーツで絞って検索し、スロットを体験予約すると確定待ちに進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    expect(find.text('スポーツを選ぶと、予約できるスロットを表示します。'), findsOneWidget);
    await tapText(tester, 'テニス');
    expect(api.requests.whereType<Sport>().last, Sport.tennis);
    expect(find.text('体験予約する'), findsOneWidget);

    await tapText(tester, '体験予約する');

    final request = api.requests.last as CreateBookingRequest;
    expect(request.toJson(), {'slot_id': Fixtures.slotId, 'is_trial': true});
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.bookingPending);
    expect(state.booking!.status, BookingStatus.pending);
  });

  testWidgets('そのスポーツのスロットがなければその旨を出す', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'サッカー');

    expect(api.requests.whereType<Sport>().last, Sport.soccer);
    expect(find.text('予約できるスロットはありません。'), findsOneWidget);
  });

  testWidgets('水泳で絞って検索すると水泳のスロットを出し、体験予約できる', (tester) async {
    final slot = Fixtures.slot;
    final swimming = Slot(
      slotId: 'slot-swimming',
      coachId: slot.coachId,
      sport: Sport.swimming,
      startsAt: slot.startsAt,
      endsAt: slot.endsAt,
    );
    final api = FakeStudentApi(slots: [slot, swimming]);
    final container = await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, '水泳');

    expect(api.requests.whereType<Sport>().last, Sport.swimming);
    expect(container.read(studentFlowProvider).searchSport, Sport.swimming);
    expect(find.text('水泳'), findsNWidgets(2), reason: '選択肢とスロットの行');
    expect(find.text('体験予約する'), findsOneWidget);

    await tapText(tester, '体験予約する');

    final request = api.requests.last as CreateBookingRequest;
    expect(request.toJson(), {'slot_id': 'slot-swimming', 'is_trial': true});
    expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);
  });

  testWidgets('先に予約されたスロットなら文言を出す', (tester) async {
    final api = FakeStudentApi(
      failures: {
        'createBooking': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.slotAlreadyBooked,
            message: 'slot already booked',
          ),
        ),
      },
    );
    final container = await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');
    await tapText(tester, '体験予約する');

    expect(find.text('このスロットはすでに予約されました。別のスロットを選んでください。'), findsOneWidget);
    expect(container.read(studentFlowProvider).step, FlowStep.slotSearch);
  });

  testWidgets('コーチの名前と自己紹介文があれば、スロットの行に出す', (tester) async {
    final api = FakeStudentApi(
      slots: [Fixtures.slotWithCoach(displayName: '佐藤', bio: 'フォアハンドが得意です。')],
    );
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    expect(find.text('コーチ: 佐藤'), findsOneWidget);
    expect(find.text('フォアハンドが得意です。'), findsOneWidget);
    expect(find.text('続きを読む'), findsNothing);
  });

  testWidgets('長い自己紹介文は 2 行までにし、押すと全文を出す', (tester) async {
    final bio = 'テニス歴二十年のコーチです。' * 20;
    final api = FakeStudentApi(
      slots: [Fixtures.slotWithCoach(displayName: '佐藤', bio: bio)],
    );
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    Text bioText() => tester.widget<Text>(find.text(bio));
    expect(bioText().maxLines, 2);
    expect(bioText().overflow, TextOverflow.ellipsis);

    await tapText(tester, '続きを読む');
    expect(bioText().maxLines, isNull);
    expect(find.text('続きを読む'), findsNothing);

    await tapText(tester, '閉じる');
    expect(bioText().maxLines, 2);
  });

  testWidgets('自己紹介文がなければ名前だけを出す', (tester) async {
    final api = FakeStudentApi(
      slots: [Fixtures.slotWithCoach(displayName: '佐藤')],
    );
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    expect(find.text('コーチ: 佐藤'), findsOneWidget);
    expect(find.text('続きを読む'), findsNothing);
  });

  testWidgets('コーチの名前も自己紹介文もなければ、今までどおりスポーツだけを出す', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    expect(find.text('テニス'), findsNWidgets(2));
    expect(find.textContaining('コーチ'), findsNothing);
  });

  testWidgets('スロットの行は日時・スポーツ・コーチ・自己紹介文・ボタンの順に縦に並べ、自己紹介文は行の幅で出す', (
    tester,
  ) async {
    final bio = 'テニス歴二十年のコーチです。' * 20;
    final slot = Fixtures.slotWithCoach(displayName: '佐藤', bio: bio);
    final api = FakeStudentApi(slots: [slot]);
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    final card = find.byType(Card);
    Finder inCard(String text) =>
        find.descendant(of: card, matching: find.text(text));
    final time = inCard(formatTimeRange(slot.startsAt, slot.endsAt));
    final order = [
      time,
      inCard('テニス'),
      inCard('コーチ: 佐藤'),
      inCard(bio),
      inCard('続きを読む'),
      inCard('体験予約する'),
    ];
    final tops = [for (final f in order) tester.getTopLeft(f).dy];
    for (var i = 1; i < tops.length; i++) {
      expect(tops[i], greaterThan(tops[i - 1]), reason: '${i + 1} 番目の並び');
    }
    expect(tester.widget<Text>(time).maxLines, isNull);
    expect(tester.getSize(time).height, lessThan(40), reason: '日時は 1 行');

    final cardWidth = tester.getSize(card).width;
    expect(tester.getSize(inCard(bio)).width, greaterThan(cardWidth * 0.8));
    final button = find.ancestor(
      of: inCard('体験予約する'),
      matching: find.byType(FilledButton),
    );
    expect(tester.getSize(button).width, greaterThan(cardWidth * 0.8));
  });

  testWidgets('グループレッスンの枠は、スポーツの下に定員と残りの席を出す', (tester) async {
    final slot = Fixtures.slot;
    final group = Slot(
      slotId: slot.slotId,
      coachId: slot.coachId,
      sport: slot.sport,
      startsAt: slot.startsAt,
      endsAt: slot.endsAt,
      capacity: 4,
      remainingSeats: 3,
    );
    final api = FakeStudentApi(slots: [group]);
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    final card = find.byType(Card);
    Finder inCard(String text) =>
        find.descendant(of: card, matching: find.text(text));
    final seats = inCard('グループ（定員 4 人・残り 3 席）');
    expect(seats, findsOneWidget);
    expect(
      tester.getTopLeft(seats).dy,
      greaterThan(tester.getTopLeft(inCard('テニス')).dy),
    );
    expect(
      tester.getTopLeft(inCard('体験予約する')).dy,
      greaterThan(tester.getTopLeft(seats).dy),
    );
  });

  testWidgets('定員 1 の枠には定員と残りの席を出さない', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const SlotSearchScreen(),
      api: api,
      initial: registered,
    );

    await tapText(tester, 'テニス');

    expect(Fixtures.slot.capacity, 1);
    expect(find.textContaining('定員'), findsNothing);
    expect(find.textContaining('席'), findsNothing);
  });

  group('コーチの評価', () {
    testWidgets('検索結果の枠に、そのコーチの評価の件数と平均を出す', (tester) async {
      final api = FakeStudentApi(
        slots: [Fixtures.slotWithCoach(displayName: '佐藤')],
      );
      await pumpScreen(
        tester,
        const SlotSearchScreen(),
        api: api,
        initial: registered,
      );

      await tapText(tester, 'テニス');

      expect(api.calls, ['searchSlots', 'getCoachReviewSummary']);
      expect(api.requests.last, Fixtures.coachId);
      final card = find.byType(Card);
      Finder inCard(String text) =>
          find.descendant(of: card, matching: find.text(text));
      final summary = inCard('★ 4.5（2 件）');
      expect(summary, findsOneWidget);
      expect(inCard('評価を見る'), findsOneWidget);
      expect(
        tester.getTopLeft(summary).dy,
        greaterThan(tester.getTopLeft(inCard('コーチ: 佐藤')).dy),
      );
      expect(
        tester.getTopLeft(inCard('体験予約する')).dy,
        greaterThan(tester.getTopLeft(summary).dy),
      );
    });

    testWidgets('押すとそのコーチの評価の一覧を新しい順に出し、もう一度押すと閉じる', (tester) async {
      final api = FakeStudentApi();
      await pumpScreen(
        tester,
        const SlotSearchScreen(),
        api: api,
        initial: registered,
      );
      await tapText(tester, 'テニス');

      await tapText(tester, '評価を見る');

      expect(api.calls.last, 'listCoachReviews');
      expect(api.requests.last, Fixtures.coachId);
      final [newer, older] = Fixtures.coachReviews;
      final texts = [
        '5 点（5 点満点）',
        newer.comment,
        formatDateTime(newer.submittedAt),
        '4 点（5 点満点）',
        older.comment,
        formatDateTime(older.submittedAt),
      ];
      final tops = [for (final t in texts) tester.getTopLeft(find.text(t)).dy];
      for (var i = 1; i < tops.length; i++) {
        expect(tops[i], greaterThan(tops[i - 1]), reason: '${texts[i]} の並び');
      }

      await tapText(tester, '評価を閉じる');
      expect(find.text(newer.comment), findsNothing);
      expect(find.text('評価を見る'), findsOneWidget);

      await tapText(tester, '評価を見る');
      expect(
        api.calls.where((c) => c == 'listCoachReviews'),
        hasLength(1),
        reason: '読んだ一覧は検索し直すまで使い回す',
      );
      expect(find.text(newer.comment), findsOneWidget);
    });

    testWidgets('評価が 0 件のコーチは、評価がまだない旨だけを出す', (tester) async {
      final api = FakeStudentApi(
        coachReviewSummaries: {
          Fixtures.coachId: const CoachReviewSummary(
            coachId: Fixtures.coachId,
            reviewCount: 0,
            averageRating: null,
          ),
        },
      );
      await pumpScreen(
        tester,
        const SlotSearchScreen(),
        api: api,
        initial: registered,
      );

      await tapText(tester, 'テニス');

      expect(find.text('評価はまだありません'), findsOneWidget);
      expect(find.text('評価を見る'), findsNothing);
    });

    testWidgets('同じコーチの枠が並んでも件数と平均は 1 回だけ読み、コーチごとに出す', (tester) async {
      const otherCoachId = '23232323-2323-4323-8323-232323232323';
      final slot = Fixtures.slot;
      Slot slotOf(String slotId, String coachId) => Slot(
        slotId: slotId,
        coachId: coachId,
        sport: slot.sport,
        startsAt: slot.startsAt,
        endsAt: slot.endsAt,
      );
      final api = FakeStudentApi(
        slots: [
          slotOf('slot-1', Fixtures.coachId),
          slotOf('slot-2', otherCoachId),
          slotOf('slot-3', Fixtures.coachId),
        ],
        coachReviewSummaries: {
          otherCoachId: const CoachReviewSummary(
            coachId: otherCoachId,
            reviewCount: 12,
            averageRating: 4.17,
          ),
        },
      );
      await pumpScreen(
        tester,
        const SlotSearchScreen(),
        api: api,
        initial: registered,
      );

      await tapText(tester, 'テニス');

      expect(
        api.calls.where((c) => c == 'getCoachReviewSummary'),
        hasLength(2),
      );
      expect(find.text('★ 4.5（2 件）'), findsNWidgets(2));
      expect(find.text('★ 4.2（12 件）'), findsOneWidget);
    });

    testWidgets('件数と平均を読めなくても検索結果は出し、評価の欄にだけその旨を出す', (tester) async {
      final api = FakeStudentApi(
        failures: {'getCoachReviewSummary': Fixtures.unreachable},
      );
      final container = await pumpScreen(
        tester,
        const SlotSearchScreen(),
        api: api,
        initial: registered,
      );

      await tapText(tester, 'テニス');

      expect(find.text('評価を読み込めませんでした。'), findsOneWidget);
      expect(find.text('評価を見る'), findsNothing);
      final state = container.read(studentFlowProvider);
      expect(state.slots, hasLength(1));
      expect(state.errorMessage, isNull);
      expect(state.busy, isFalse);

      await tapText(tester, '体験予約する');
      expect(container.read(studentFlowProvider).step, FlowStep.bookingPending);
    });

    testWidgets('評価の一覧を読めなければその旨を出し、読み直せる。検索結果はそのまま使える', (tester) async {
      final failures = {'listCoachReviews': Fixtures.unreachable};
      final api = FakeStudentApi(failures: failures);
      final container = await pumpScreen(
        tester,
        const SlotSearchScreen(),
        api: api,
        initial: registered,
      );
      await tapText(tester, 'テニス');

      await tapText(tester, '評価を見る');

      expect(find.text('評価の一覧を読み込めませんでした。'), findsOneWidget);
      expect(find.text('★ 4.5（2 件）'), findsOneWidget);
      expect(container.read(studentFlowProvider).errorMessage, isNull);
      expect(find.text('体験予約する'), findsOneWidget);

      failures.clear();
      await tapText(tester, 'もう一度読み込む');

      expect(api.calls.where((c) => c == 'listCoachReviews'), hasLength(2));
      expect(find.text('評価の一覧を読み込めませんでした。'), findsNothing);
      expect(find.text(Fixtures.coachReviews.first.comment), findsOneWidget);
    });
  });
}
