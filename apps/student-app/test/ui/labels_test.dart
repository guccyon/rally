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

  test('月額契約のプランは名前とビデオ通話の暦月の上限で出す', () {
    expect(SubscriptionPlan.values.map(subscriptionPlanChoiceLabel), [
      'スタンダード　ビデオ通話 月 2 回',
      'プラス　ビデオ通話 月 4 回',
    ]);
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
}
