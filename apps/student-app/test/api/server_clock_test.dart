import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/server_clock.dart';

void main() {
  final sentAt = DateTime.utc(2026, 11, 10, 10);
  final receivedAt = DateTime.utc(2026, 11, 10, 10, 0, 0, 200);

  test('ずれは Date の時刻 −（送った時刻と受け取った時刻の中間）', () {
    expect(
      clockOffsetOf(
        'Tue, 10 Nov 2026 10:02:00 GMT',
        sentAt: sentAt,
        receivedAt: receivedAt,
      ),
      const Duration(minutes: 2, milliseconds: -100),
    );
  });

  test('Date がない・読めないときは null', () {
    expect(clockOffsetOf(null, sentAt: sentAt, receivedAt: receivedAt), isNull);
    expect(clockOffsetOf('', sentAt: sentAt, receivedAt: receivedAt), isNull);
    expect(
      clockOffsetOf(
        '2026-11-10T10:02:00Z',
        sentAt: sentAt,
        receivedAt: receivedAt,
      ),
      isNull,
    );
  });

  test('サーバーに合わせた時計は、端末の時計にずれを足す。null では合わせ直さない', () {
    final device = DateTime.utc(2026, 11, 10, 10);
    final clock = ServerClock(() => device);
    expect(clock.now(), device);

    final adjusted = clock.adjustedTo(const Duration(minutes: 2));
    expect(adjusted.now(), device.add(const Duration(minutes: 2)));
    expect(
      adjusted.adjustedTo(null).now(),
      device.add(const Duration(minutes: 2)),
    );
    expect(
      adjusted.adjustedTo(const Duration(minutes: -2)).now(),
      device.subtract(const Duration(minutes: 2)),
    );
  });
}
