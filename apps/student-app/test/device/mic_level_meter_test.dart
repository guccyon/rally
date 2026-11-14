import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/device/ios_local_media.dart';

/// 振幅 [amplitude]（0.0〜1.0）の正弦波を 16bit・リトルエンディアンの PCM にする
Uint8List _sine(double amplitude, {int samples = 1600}) {
  final data = ByteData(samples * 2);
  for (var i = 0; i < samples; i++) {
    final s = amplitude * math.sin(2 * math.pi * 440 * i / 16000);
    data.setInt16(i * 2, (s * 32767).round(), Endian.little);
  }
  return data.buffer.asUint8List();
}

void main() {
  test('音が届いていなければ大きさを出さない', () {
    expect(MicLevelMeter().take(), isNull);
  });

  test('無音は 0、大きな音は 1 に寄せる', () {
    final meter = MicLevelMeter()..add(_sine(0));
    expect(meter.take(), 0);
    meter.add(_sine(1));
    expect(meter.take(), 1);
  });

  test('目盛りは Web 版と同じ（-60dB が 0、-10dB が 1）', () {
    // 振幅 a の正弦波の実効値は a/√2。-35dB（中ほど）になる振幅
    final amplitude = math.pow(10, -35 / 20) * math.sqrt2;
    final meter = MicLevelMeter()..add(_sine(amplitude.toDouble()));
    expect(meter.take(), closeTo(0.5, 0.01));
  });

  test('取ったら貯めた音を捨て、次の間隔は新しく届いた音だけで計る', () {
    final meter = MicLevelMeter()..add(_sine(1));
    expect(meter.take(), 1);
    expect(meter.take(), isNull);
    meter
      ..add(_sine(1))
      ..reset();
    expect(meter.take(), isNull);
  });

  test('いくつかに分かれて届いた音をまとめて計る（端の 1 バイトは捨てる）', () {
    final whole = _sine(0.1);
    final meter = MicLevelMeter()
      ..add(Uint8List.sublistView(whole, 0, 1001))
      ..add(Uint8List.sublistView(whole, 1002));
    final expected = (MicLevelMeter()..add(whole)).take()!;
    expect(meter.take(), closeTo(expected, 0.01));
  });
}
