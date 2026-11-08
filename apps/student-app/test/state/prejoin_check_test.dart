import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/state/prejoin_check.dart';

import '../support/fake_student_api.dart';

void main() {
  test('入室前チェックを開けるのは開始 5 分前ちょうどから、入室できるのは開始時刻ちょうどから', () {
    final startsAt = Fixtures.slot.startsAt;
    const second = Duration(seconds: 1);
    expect(prejoinCheckLeadTime, const Duration(minutes: 5));
    final opensAt = prejoinCheckOpensAt(startsAt);
    expect(opensAt, startsAt.subtract(prejoinCheckLeadTime));
    expect(canOpenPrejoinCheck(startsAt, opensAt.subtract(second)), isFalse);
    expect(canOpenPrejoinCheck(startsAt, opensAt), isTrue);
    expect(canJoinLiveLesson(startsAt, startsAt.subtract(second)), isFalse);
    expect(canJoinLiveLesson(startsAt, startsAt), isTrue);
  });
}
