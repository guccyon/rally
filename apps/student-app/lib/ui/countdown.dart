/// 開始までの残り時間の表記（specs/design/content.yaml の notation.datetime.countdown。0.7.0）。
/// 残りはサーバーの時刻に合わせた時計で数える（端末の時計を使わない）。数えるのは呼ぶ側。
library;

/// 残りの秒（端数は切り上げ。2:47.2 は 2:48 と数える）
int _seconds(Duration remaining) => (remaining.inMilliseconds / 1000).ceil();

/// 「あと 2:48」の「2:48」（分:秒、秒は 2 桁）。1 時間未満の残りだけで、それ以外（0 以下・1 時間以上）は null。
String? formatCountdown(Duration remaining) {
  final total = _seconds(remaining);
  if (total <= 0 || total >= 3600) return null;
  return '${total ~/ 60}:${(total % 60).toString().padLeft(2, '0')}';
}

/// 読み上げの文。「開始まで あと 2 分 48 秒」。0 以下は「開始時刻になりました。入室できます」。0 の分・秒は省く。
String countdownReadAloud(Duration remaining) {
  final total = _seconds(remaining);
  if (total <= 0) return '開始時刻になりました。入室できます';
  final minutes = total ~/ 60;
  final seconds = total % 60;
  final parts = [
    if (minutes > 0) '$minutes 分',
    if (seconds > 0) '$seconds 秒',
  ];
  return '開始まで あと ${parts.join(' ')}';
}

/// 読み上げ（liveRegion）に置く文。1 分ごと（秒が 0 になったとき）と 0 になったときだけ変わるので、変わったときに 1 回読まれる。
/// 分の途中は、直前に読んだ分の文のまま（2:48 のあいだは「開始まで あと 3 分」）。1 時間以上は読まない（空）。
String countdownAnnouncement(Duration remaining) {
  final total = _seconds(remaining);
  if (total >= 3600) return '';
  final boundary = total <= 0 ? 0 : (total / 60).ceil() * 60;
  return countdownReadAloud(Duration(seconds: boundary));
}
