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
    expect(api.requests.last, Sport.tennis);
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

    expect(api.requests.last, Sport.soccer);
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

    expect(api.requests.last, Sport.swimming);
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
}
