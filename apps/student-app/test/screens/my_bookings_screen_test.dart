import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';
import 'package:rally_student_app/state/subscription.dart';
import 'package:rally_student_app/ui/labels.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final afterSubscription = StudentFlowState(
    step: FlowStep.subscription,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );
  final subscribed = SubscriptionState(
    subscription: Fixtures.subscriptionStarted.subscription,
    entitlement: Fixtures.subscriptionStarted.entitlement,
  );

  /// 月額契約の前（受講権がない）
  final beforeSubscription = StudentFlowState(
    step: FlowStep.slotSearch,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  const quotaNotice = cancelConsumesQuotaNotice;
  const cancelLabel = 'この予約をキャンセルする';

  MyBooking bookingOf(String bookingId) =>
      Fixtures.myBookings.firstWhere((b) => b.booking.bookingId == bookingId);

  String timeOf(MyBooking item) =>
      formatTimeRange(item.slot.startsAt, item.slot.endsAt);

  /// 一覧の 1 件（日時の行で探す）
  Finder cardOf(MyBooking item) => find.widgetWithText(Card, timeOf(item));

  /// キャンセルの確認の中の文
  Finder inDialog(String text) =>
      find.descendant(of: find.byType(AlertDialog), matching: find.text(text));

  /// 行の中の文
  Finder inCard(MyBooking item, String text) =>
      find.descendant(of: cardOf(item), matching: find.text(text));

  Future<void> openBookings(WidgetTester tester) async {
    await tester.tap(find.byTooltip('自分の予約'));
    await tester.pumpAndSettle();
  }

  Future<void> tapCancelOf(WidgetTester tester, MyBooking item) async {
    final button = find.descendant(
      of: cardOf(item),
      matching: find.widgetWithText(OutlinedButton, cancelLabel),
    );
    await tester.ensureVisible(button);
    await tester.tap(button);
    await tester.pumpAndSettle();
  }

  testWidgets(
    'アプリバーから開くと listMyBookings で読み込み、日時・スポーツ・コーチ・状態を出し、閉じると開いた画面に戻る',
    (tester) async {
      final api = FakeStudentApi();
      final container = await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: api,
        initial: afterSubscription,
        subscription: subscribed,
      );

      await openBookings(tester);

      expect(api.calls, ['listMyBookings']);
      expect(find.text('自分の予約'), findsOneWidget);
      expect(find.byType(Card), findsNWidgets(4));
      for (final (id, sport, coach, status) in [
        ('booking-later', 'テニス', 'コーチ: 佐藤', '状態: 確定済み'),
        ('booking-soon', 'サッカー', 'コーチ: 鈴木', '状態: 確定待ち'),
        ('booking-cancelled', 'テニス', 'コーチ: 佐藤', '状態: キャンセル済み'),
        ('booking-past', 'テニス', 'コーチ: 佐藤', '状態: 確定済み'),
      ]) {
        final card = cardOf(bookingOf(id));
        expect(card, findsOneWidget, reason: id);
        for (final text in [sport, coach, status]) {
          expect(
            find.descendant(of: card, matching: find.text(text)),
            findsOneWidget,
            reason: '$id: $text',
          );
        }
      }
      // まだ始まっていない予約（キャンセル済みを除く）だけにキャンセルのボタン
      for (final (id, cancellable) in [
        ('booking-later', true),
        ('booking-soon', true),
        ('booking-cancelled', false),
        ('booking-past', false),
      ]) {
        expect(
          find.descendant(
            of: cardOf(bookingOf(id)),
            matching: find.text(cancelLabel),
          ),
          cancellable ? findsOneWidget : findsNothing,
          reason: id,
        );
      }
      // 自分の予約の画面からはほかの脇の画面を重ねて開かない
      expect(find.byTooltip('チャット'), findsNothing);
      expect(find.byTooltip('通知の設定'), findsNothing);
      expect(find.byTooltip('自分の予約'), findsNothing);

      await tester.tap(find.byTooltip('自分の予約を閉じる'));
      await tester.pumpAndSettle();
      expect(container.read(studentFlowProvider).step, FlowStep.subscription);
    },
  );

  testWidgets('予約した時点のレッスンメニューの名前と 1 回の時間を、スポーツとコーチの間に出す。メニューのない予約には出さない', (
    tester,
  ) async {
    final withMenu = Fixtures.myBooking(
      bookingId: 'booking-menu',
      startsIn: const Duration(days: 3),
      lessonMenu: Fixtures.bookingLessonMenu,
    );
    final asyncMenu = Fixtures.myBooking(
      bookingId: 'booking-async-menu',
      startsIn: const Duration(days: 2),
      lessonMenu: const BookingLessonMenu(
        lessonMenuId: '13131313-1313-4313-8313-131313131313',
        name: 'サーブの動画添削',
      ),
    );
    final noMenu = Fixtures.myBooking(
      bookingId: 'booking-no-menu',
      startsIn: const Duration(days: 1),
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: FakeStudentApi(myBookings: [withMenu, asyncMenu, noMenu]),
      initial: afterSubscription,
      subscription: subscribed,
    );

    await openBookings(tester);

    Finder inCard(MyBooking item, String text) =>
        find.descendant(of: cardOf(item), matching: find.text(text));
    final menu = inCard(withMenu, 'メニュー: フォアハンドの基礎（1 回 60 分）');
    expect(menu, findsOneWidget);
    final order = [
      inCard(withMenu, 'テニス'),
      menu,
      inCard(withMenu, 'コーチ: 佐藤'),
    ].map((f) => tester.getTopLeft(f).dy).toList();
    expect(order, [...order]..sort());
    // 予約した時点で非同期に直されていたメニューは名前だけ
    expect(inCard(asyncMenu, 'メニュー: サーブの動画添削'), findsOneWidget);
    expect(
      find.descendant(
        of: cardOf(noMenu),
        matching: find.textContaining('メニュー:'),
      ),
      findsNothing,
    );
  });

  testWidgets('コーチの名前がない予約は名前の行を出さず、予約がなければそう伝える', (tester) async {
    final api = FakeStudentApi(
      myBookings: [
        Fixtures.myBooking(
          bookingId: 'booking-no-coach',
          startsIn: const Duration(days: 3),
          coachDisplayName: null,
        ),
      ],
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);

    expect(find.textContaining('コーチ:'), findsNothing);
    expect(find.text('テニス'), findsOneWidget);

    api.myBookings.clear();
    await tapText(tester, '一覧を更新する');
    expect(api.calls, ['listMyBookings', 'listMyBookings']);
    expect(find.byType(Card), findsNothing);
    expect(find.text('まだ予約はありません。スロットを探して予約すると、ここに出ます。'), findsOneWidget);
  });

  testWidgets('24 時間より先の予約は、確認してから cancelBooking でキャンセルし、状態をキャンセル済みにする', (
    tester,
  ) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);
    final later = bookingOf('booking-later');

    await tapCancelOf(tester, later);

    expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
    expect(find.text('${timeOf(later)} のテニスの予約をキャンセルします。'), findsOneWidget);
    expect(inDialog(quotaNotice), findsNothing);
    expect(api.calls, ['listMyBookings']);

    await tapText(tester, 'キャンセルする');

    expect(api.calls, ['listMyBookings', 'cancelBooking']);
    expect(api.requests.single, 'booking-later');
    expect(find.text('予約をキャンセルしますか？'), findsNothing);
    final card = cardOf(later);
    expect(
      find.descendant(of: card, matching: find.text('状態: キャンセル済み')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: card, matching: find.text(cancelLabel)),
      findsNothing,
    );
    expect(
      container
          .read(studentFlowProvider)
          .myBookings!
          .firstWhere((b) => b.booking.bookingId == 'booking-later')
          .booking
          .status,
      BookingStatus.cancelled,
    );
    // ほかの予約はそのまま
    expect(find.text(cancelLabel), findsOneWidget);
  });

  testWidgets('24 時間を切った予約は、確認に今月のビデオ通話の回数を 1 回使うことを添え、やめればキャンセルしない', (
    tester,
  ) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);
    final soon = bookingOf('booking-soon');

    await tapCancelOf(tester, soon);

    expect(find.text('${timeOf(soon)} のサッカーの予約をキャンセルします。'), findsOneWidget);
    expect(inDialog(quotaNotice), findsOneWidget);

    await tapText(tester, 'やめる');

    expect(api.calls, ['listMyBookings']);
    expect(find.text('予約をキャンセルしますか？'), findsNothing);
    expect(
      find.descendant(of: cardOf(soon), matching: find.text('状態: 確定待ち')),
      findsOneWidget,
    );

    await tapCancelOf(tester, soon);
    await tapText(tester, 'キャンセルする');
    expect(api.calls, ['listMyBookings', 'cancelBooking']);
    expect(api.requests.single, 'booking-soon');
    expect(
      find.descendant(of: cardOf(soon), matching: find.text('状態: キャンセル済み')),
      findsOneWidget,
    );
  });

  for (final (title, before, notice) in [
    ('開始のちょうど 24 時間前は回数を使うことを添える', Duration(hours: 24), true),
    ('開始の 24 時間と 1 分前なら回数のことは添えない', Duration(hours: 24, minutes: 1), false),
  ]) {
    testWidgets(title, (tester) async {
      final later = bookingOf('booking-later');
      await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: FakeStudentApi(),
        initial: afterSubscription,
        subscription: subscribed,
        now: later.booking.startsAt.subtract(before),
      );
      await openBookings(tester);

      await tapCancelOf(tester, later);

      expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
      expect(inDialog(quotaNotice), notice ? findsOneWidget : findsNothing);
    });
  }

  testWidgets(
    '月額の受講権のない生徒の予約（cancel_consumes_quota が false・free_cancel_until が null）は、24 時間を切っていても行にも確認にも枠のことを出さない',
    (tester) async {
      final soon = Fixtures.myBooking(
        bookingId: 'booking-trial-soon',
        startsIn: const Duration(hours: 1),
        subscribed: false,
      );
      await pumpScreen(
        tester,
        const StudentFlowScreen(),
        api: FakeStudentApi(myBookings: [soon]),
        initial: beforeSubscription,
      );
      await openBookings(tester);

      expect(find.text(cancelLabel), findsOneWidget);
      expect(find.text(quotaNotice), findsNothing);
      expect(find.textContaining('枠を使わずに'), findsNothing);

      await tapCancelOf(tester, soon);

      expect(find.text('予約をキャンセルしますか？'), findsOneWidget);
      expect(find.text(quotaNotice), findsNothing);
    },
  );

  testWidgets('境目の前の予約の行には、枠を使わずにキャンセルできる日時を出し、確認には枠のことを添えない', (tester) async {
    final later = bookingOf('booking-later');
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: FakeStudentApi(),
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);

    final until = later.freeCancelUntil!.toLocal();
    final label =
        '${until.month}月${until.day}日 ${until.hour}時までなら、'
        'ビデオ通話の枠を使わずにキャンセルできます。';
    expect(inCard(later, label), findsOneWidget);
    expect(inCard(later, quotaNotice), findsNothing);

    await tapCancelOf(tester, later);
    expect(inDialog(quotaNotice), findsNothing);
  });

  testWidgets('境目を過ぎた予約の行には、今キャンセルすると今月の枠を 1 回使うことを出し、確認にも添える', (tester) async {
    final soon = bookingOf('booking-soon');
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: FakeStudentApi(),
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);

    expect(inCard(soon, quotaNotice), findsOneWidget);
    expect(
      find.descendant(
        of: cardOf(soon),
        matching: find.textContaining('枠を使わずに'),
      ),
      findsNothing,
    );

    await tapCancelOf(tester, soon);
    expect(inDialog(quotaNotice), findsOneWidget);
  });

  testWidgets('一覧を読んだあとで境目を過ぎたら、今の時刻で枠を使う側の案内にする', (tester) async {
    // 一覧は Fixtures.now の見込み（境目の前で cancel_consumes_quota が false）のまま、今は境目ちょうど
    final later = bookingOf('booking-later');
    expect(later.cancelConsumesQuota, isFalse);
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: FakeStudentApi(),
      initial: afterSubscription,
      subscription: subscribed,
      now: later.freeCancelUntil,
    );
    await openBookings(tester);

    expect(inCard(later, quotaNotice), findsOneWidget);
    expect(
      find.descendant(
        of: cardOf(later),
        matching: find.textContaining('枠を使わずに'),
      ),
      findsNothing,
    );
  });

  testWidgets('境目が null の予約（キャンセル済み・始まった予約）には枠の案内を出さない', (tester) async {
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: FakeStudentApi(),
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);

    final cancelled = bookingOf('booking-cancelled');
    expect(cancelled.cancelConsumesQuota, isNull);
    expect(cancelled.freeCancelUntil, isNull);
    for (final id in ['booking-cancelled', 'booking-past']) {
      final card = cardOf(bookingOf(id));
      expect(
        find.descendant(of: card, matching: find.textContaining('枠を')),
        findsNothing,
        reason: id,
      );
    }
  });

  testWidgets('キャンセルに失敗したら状態を変えずに知らせ、やり直せる', (tester) async {
    final api = FakeStudentApi(
      failures: {
        'cancelBooking': const StudentApiException(
          statusCode: 409,
          error: ApiError(
            code: ErrorCode.invalidBookingState,
            message: 'invalid booking state',
          ),
        ),
      },
    );
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);
    final later = bookingOf('booking-later');

    await tapCancelOf(tester, later);
    await tapText(tester, 'キャンセルする');

    expect(api.calls, ['listMyBookings', 'cancelBooking']);
    // 失敗の文言は一覧の先頭に出る（ボタンを押すときに ensureVisible で下へ送ったので戻す）
    await tester.drag(find.byType(ListView), const Offset(0, 600));
    await tester.pumpAndSettle();
    expect(find.text('予約の状態が変わっています。予約を確かめてください。'), findsOneWidget);
    expect(
      find.descendant(of: cardOf(later), matching: find.text('状態: 確定済み')),
      findsOneWidget,
    );
    final button = tester.widget<OutlinedButton>(
      find.descendant(
        of: cardOf(later),
        matching: find.widgetWithText(OutlinedButton, cancelLabel),
      ),
    );
    expect(button.onPressed, isNotNull);
  });

  testWidgets('読み込みに失敗したら知らせ、読み込み直せる', (tester) async {
    final failures = {'listMyBookings': Fixtures.unreachable};
    final api = FakeStudentApi(failures: failures);
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: afterSubscription,
      subscription: subscribed,
    );
    await openBookings(tester);

    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsOneWidget);
    expect(find.textContaining('状態:'), findsNothing);

    failures.clear();
    await tapText(tester, '一覧を更新する');

    expect(api.calls, ['listMyBookings', 'listMyBookings']);
    expect(find.byType(Card), findsNWidgets(4));
    expect(find.text('サーバーにつながりませんでした。通信状態を確かめてください。'), findsNothing);
  });
}
