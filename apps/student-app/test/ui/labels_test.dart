import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/ui/labels.dart';

void main() {
  test('評価の各点に言葉の目安を添える', () {
    expect(
      [for (var rating = 1; rating <= 5; rating++) ratingLabel(rating)],
      ['不満だった', '少し物足りなかった', 'ふつう', 'よかった', 'とてもよかった'],
    );
    expect(ratingWithLabel(4), '4（よかった）');
  });

  test('契約の範囲（1〜5）の外の点は受け付けない', () {
    expect(() => ratingLabel(0), throwsRangeError);
    expect(() => ratingLabel(6), throwsRangeError);
  });
}
