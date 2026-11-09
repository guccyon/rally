import 'package:http_parser/http_parser.dart';

/// 応答の Date ヘッダーから、サーバーの時計と端末の時計のずれ（サーバー − 端末）を出す。
/// 端末の時刻は送った時刻 [sentAt] と受け取った時刻 [receivedAt] の中間とみなす。Date がない・読めないときは null（呼ぶ側は今のずれのまま）。
/// Date は秒までなので、サーバーの時刻を切り捨てた側に寄せる（開始時刻より前に押せるほうへはずらさない）。
Duration? clockOffsetOf(
  String? dateHeader, {
  required DateTime sentAt,
  required DateTime receivedAt,
}) {
  if (dateHeader == null) return null;
  final DateTime serverTime;
  try {
    serverTime = parseHttpDate(dateHeader);
  } on FormatException {
    return null;
  }
  final deviceTime = sentAt.add(receivedAt.difference(sentAt) ~/ 2);
  return serverTime.difference(deviceTime);
}

/// サーバーに合わせた時計。端末の時計 [deviceNow] に、応答の Date から出したずれ [offset] を足して今の時刻とする。
/// ずれの初めは 0（Date を一度も読めていなければ端末の時計のまま）。
class ServerClock {
  const ServerClock(this.deviceNow, [this.offset = Duration.zero]);

  /// 端末の時計（テストで差し替える）
  final DateTime Function() deviceNow;

  /// サーバーの時計 − 端末の時計
  final Duration offset;

  /// サーバーに合わせた今の時刻
  DateTime now() => deviceNow().add(offset);

  /// ずれを [offset] に合わせ直した時計。null（Date がない・読めない）なら今のまま
  ServerClock adjustedTo(Duration? offset) =>
      offset == null ? this : ServerClock(deviceNow, offset);
}
