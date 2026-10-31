import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/ui/labels.dart';

void main() {
  test('スポーツの表示名は、契約の順に水泳まで日本語で出す', () {
    expect(Sport.values.map(sportLabel), [
      'テニス',
      '野球',
      'サッカー',
      'バスケットボール',
      '水泳',
    ]);
    expect(sportLabel(Sport.swimming), '水泳');
  });

  test('添削の状態は、添削待ちと返却済みで出す', () {
    expect(VideoReviewStatus.values.map(videoReviewStatusLabel), [
      '添削待ち',
      '返却済み',
    ]);
  });

  test('コーチの名前は「〇〇 コーチ」で、名前がわからなければ「担当コーチ」', () {
    expect(coachNameLabel('佐藤'), '佐藤 コーチ');
    expect(coachNameLabel(null), '担当コーチ');
  });

  test('評価の各点に言葉の目安を添える', () {
    expect(
      [for (var rating = 1; rating <= 5; rating++) ratingLabel(rating)],
      ['不満だった', '少し物足りなかった', 'ふつう', 'よかった', 'とてもよかった'],
    );
    expect(ratingWithLabel(4), '4（よかった）');
    expect(ratingChoiceLabel(5), '5　とてもよかった');
  });

  test('契約の範囲（1〜5）の外の点は受け付けない', () {
    expect(() => ratingLabel(0), throwsRangeError);
    expect(() => ratingLabel(6), throwsRangeError);
  });

  test('スタンプは 8 種類すべてに絵文字と、契約の呼び名の短い言葉を持つ', () {
    expect(StampId.values.map(stampLabel), [
      'ありがとう',
      'いいね',
      '了解',
      'がんばろう',
      'ナイスショット',
      'おつかれさま',
      '質問',
      'OK',
    ]);
    expect(stampEmoji(StampId.thanks), '🙏');
    // 絵文字は種類ごとに見分けられる
    expect(StampId.values.map(stampEmoji).toSet(), hasLength(8));
  });

  test('通知の種類は、いつ届く通知かで書く', () {
    expect(NotificationKind.values.map(notificationKindLabel), [
      '予約が確定したとき',
      '動画の添削が返ってきたとき',
      '担当コーチからメッセージが届いたとき',
    ]);
  });

  test('スロットの日時は曜日付きの 1 行にし、別の日に終わるときだけ終わりにも日付を付ける', () {
    expect(
      formatTimeRange(DateTime(2026, 10, 28, 3), DateTime(2026, 10, 28, 4)),
      '10/28（水）3:00〜4:00',
    );
    expect(
      formatTimeRange(
        DateTime(2026, 11, 1, 18, 30),
        DateTime(2026, 11, 1, 19, 5),
      ),
      '11/1（日）18:30〜19:05',
    );
    expect(
      formatTimeRange(
        DateTime(2026, 10, 28, 23, 30),
        DateTime(2026, 10, 29, 0, 30),
      ),
      '10/28（水）23:30〜10/29（木）0:30',
    );
  });

  test('月額契約のプランの選択肢は、名前と応答のビデオ通話の暦月の上限で出す', () {
    expect(
      subscriptionPlanChoiceLabel(
        const SubscriptionPlanDetail(
          plan: SubscriptionPlan.standard,
          liveLessonsPerMonth: 2,
        ),
      ),
      'スタンダード　ビデオ通話 月 2 回',
    );
    expect(
      subscriptionPlanChoiceLabel(
        const SubscriptionPlanDetail(
          plan: SubscriptionPlan.plus,
          liveLessonsPerMonth: 4,
        ),
      ),
      'プラス　ビデオ通話 月 4 回',
    );
  });

  test('今月のビデオ通話の残り回数は、プランの上限と合わせて出す', () {
    expect(
      liveLessonQuotaLabel(
        const LiveLessonQuota(
          month: '2026-10',
          limit: 4,
          used: 1,
          remaining: 3,
        ),
      ),
      '月 4 回のうち残り 3 回',
    );
  });

  test('枠のレッスンメニューは名前と 1 回の時間で出し、時間がなければ名前だけを出す', () {
    expect(
      slotLessonMenuLabel(
        const SlotLessonMenu(
          lessonMenuId: 'm1',
          name: 'フォアハンドの基礎',
          durationMinutes: 60,
        ),
      ),
      'メニュー: フォアハンドの基礎（1 回 60 分）',
    );
    expect(
      slotLessonMenuLabel(
        const SlotLessonMenu(lessonMenuId: 'm2', name: 'サーブの動画添削'),
      ),
      'メニュー: サーブの動画添削',
    );
  });

  test('予約した時点のレッスンメニューは枠と同じ書き方で、時間がなければ名前だけを出す', () {
    expect(
      bookingLessonMenuLabel(
        const BookingLessonMenu(
          lessonMenuId: 'm1',
          name: 'フォアハンドの基礎',
          durationMinutes: 60,
        ),
      ),
      'メニュー: フォアハンドの基礎（1 回 60 分）',
    );
    expect(
      bookingLessonMenuLabel(
        const BookingLessonMenu(lessonMenuId: 'm2', name: 'サーブの動画添削'),
      ),
      'メニュー: サーブの動画添削',
    );
  });

  test('定員 2 以上の枠はグループとして定員と残りの席を出し、定員 1 の枠には出さない', () {
    Slot slot({required int capacity, required int remainingSeats}) => Slot(
      slotId: 'slot-1',
      coachId: 'coach-1',
      sport: Sport.tennis,
      startsAt: DateTime.utc(2026, 10, 28, 3),
      endsAt: DateTime.utc(2026, 10, 28, 4),
      capacity: capacity,
      remainingSeats: remainingSeats,
    );
    expect(
      slotSeatsLabel(slot(capacity: 4, remainingSeats: 2)),
      'グループ（定員 4 人・残り 2 席）',
    );
    expect(
      slotSeatsLabel(slot(capacity: 2, remainingSeats: 0)),
      'グループ（定員 2 人・残り 0 席）',
    );
    expect(slotSeatsLabel(slot(capacity: 1, remainingSeats: 1)), isNull);
  });

  test('コーチの評価の件数と平均は小数第 1 位までで出し、0 件なら評価がまだない旨を出す', () {
    CoachReviewSummary summary(int count, double? average) =>
        CoachReviewSummary(
          coachId: 'coach-1',
          reviewCount: count,
          averageRating: average,
        );
    expect(coachReviewSummaryLabel(summary(12, 4.2)), '★ 4.2（12 件）');
    expect(coachReviewSummaryLabel(summary(4, 4.25)), '★ 4.3（4 件）');
    expect(coachReviewSummaryLabel(summary(3, 4.67)), '★ 4.7（3 件）');
    expect(coachReviewSummaryLabel(summary(1, 5)), '★ 5.0（1 件）');
    expect(coachReviewSummaryLabel(summary(0, null)), '評価はまだありません');
  });

  test('コーチを探す一覧の行は評価を同じ書き方で出し、0 件なら「評価なし」、メニューは件数で出す', () {
    CoachListing coach(int count, double? average, int menus) => CoachListing(
      coachId: 'c1',
      displayName: '佐藤',
      sport: Sport.tennis,
      reviewCount: count,
      averageRating: average,
      publishedLessonMenuCount: menus,
    );
    expect(coachListingRatingLabel(coach(12, 4.25, 2)), '★ 4.3（12 件）');
    expect(coachListingRatingLabel(coach(1, 5, 0)), '★ 5.0（1 件）');
    expect(coachListingRatingLabel(coach(0, null, 0)), '評価なし');
    expect(coachListingLessonMenuCountLabel(coach(0, null, 3)), 'レッスンメニュー 3 件');
    expect(coachListingLessonMenuCountLabel(coach(0, null, 0)), 'レッスンメニュー 0 件');
  });

  test('お気に入りのコーチの行は提供スポーツを「、」でつなぎ、評価を同じ書き方で出す（0 件なら「評価なし」）', () {
    FavoriteCoach coach(List<Sport> sports, int count, double? average) =>
        FavoriteCoach(
          coachId: 'c1',
          displayName: '佐藤',
          sports: sports,
          reviewCount: count,
          averageRating: average,
          favoritedAt: DateTime.utc(2026, 10, 26),
        );
    expect(
      favoriteCoachSummaryLabel(
        coach([Sport.tennis, Sport.baseball], 12, 4.25),
      ),
      'テニス、野球・★ 4.3（12 件）',
    );
    expect(
      favoriteCoachSummaryLabel(coach([Sport.swimming], 0, null)),
      '水泳・評価なし',
    );
  });

  test('お気に入りの切り替えのボタンは押すとどうなるかで書く', () {
    expect(favoriteToggleLabel(false), 'お気に入りに入れる');
    expect(favoriteToggleLabel(true), 'お気に入りから外す');
  });

  test('コーチが受けた評価の点は 5 点満点を添えて出す', () {
    expect(coachReviewRatingLabel(4), '4 点（5 点満点）');
  });

  test('評価したレッスンのメニューは名前だけを出す', () {
    expect(
      reviewLessonMenuLabel(
        const ReviewLessonMenu(lessonMenuId: 'm1', name: 'フォアハンドの基礎'),
      ),
      'メニュー: フォアハンドの基礎',
    );
  });

  test('受けたレッスンのメニューは名前だけを出す', () {
    expect(
      lessonCompletionMenuLabel(
        const LessonCompletionMenu(lessonMenuId: 'm1', name: 'サーブの動画添削'),
      ),
      'メニュー: サーブの動画添削',
    );
  });

  test('レッスンメニューの同期レッスン 1 回の時間を分で出す', () {
    expect(lessonMenuDurationLabel(60), '1 回 60 分');
    expect(lessonMenuDurationLabel(15), '1 回 15 分');
  });

  test('枠を使わずにキャンセルできる境目の案内は、端末のタイムゾーンの月日と時で出し、分があれば分まで出す', () {
    expect(
      freeCancelUntilLabel(DateTime(2026, 10, 28, 3)),
      '10月28日 3時までなら、ビデオ通話の枠を使わずにキャンセルできます。',
    );
    expect(
      freeCancelUntilLabel(DateTime(2026, 10, 28, 15, 30)),
      '10月28日 15時30分までなら、ビデオ通話の枠を使わずにキャンセルできます。',
    );
  });
}
