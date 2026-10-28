import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/ui/labels.dart';

void main() {
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
}
