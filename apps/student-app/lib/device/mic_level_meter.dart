import 'dart:math' as math;
import 'dart:typed_data';

/// マイクから届いた音（16bit・リトルエンディアンの PCM）をためて、[take] で 0.0〜1.0 の音の大きさにする。
/// 目盛りは Web 版と同じ（-60dB を 0、-10dB を 1。ふつうの声が中ほどに来る）。音はためた二乗の和だけを持ち、音そのものは残さない。
class MicLevelMeter {
  var _sumOfSquares = 0.0;
  var _count = 0;

  /// 届いた音を足す（端の 1 バイトは捨てる）
  void add(Uint8List pcm16le) {
    final data = ByteData.sublistView(pcm16le);
    for (var i = 0; i + 1 < pcm16le.length; i += 2) {
      final s = data.getInt16(i, Endian.little) / 32768;
      _sumOfSquares += s * s;
    }
    _count += pcm16le.length ~/ 2;
  }

  /// 前に取ってから届いた音の大きさ。届いていなければ null
  double? take() {
    if (_count == 0) return null;
    final rms = math.sqrt(_sumOfSquares / _count);
    reset();
    final db = 20 * math.log(math.max(rms, 1e-8)) / math.ln10;
    return ((db + 60) / 50).clamp(0.0, 1.0);
  }

  /// ためた音を捨てる
  void reset() {
    _sumOfSquares = 0;
    _count = 0;
  }
}
