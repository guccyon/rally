import 'package:flutter/material.dart';

import '../countdown.dart';
import '../theme/rally_tokens.dart';
import 'notice.dart';
import 'rally_button.dart';

/// 入室の帯（components.yaml の join_bar。0.7.0）。画面の下に固定する操作の帯で、Scaffold の bottomNavigationBar に置く。
/// 面は background・上に border の 1px・padding md（下は安全領域 + md）・影なし。「入室する」は幅いっぱい。
/// 開始時刻の前（[remaining] が 0 より大きい）は「入室する」を無効にし、理由と残り時間（1 時間未満だけ「あと 2:48」）を出す（押してから断らない）。
/// 開始時刻になったら開き直さずに押せるようにし、「開始時刻になりました。入室できます」を読み上げで 1 回伝える。
/// [remaining] はサーバーの時刻に合わせた時計で数える（呼ぶ側）。
/// 入室に失敗したら [error] にサーバーの文言を渡す（ボタンの上に notice の inline（danger）。ボタンは押せるまま）。
class JoinBar extends StatelessWidget {
  const JoinBar({
    super.key,
    required this.startTime,
    required this.remaining,
    required this.onJoin,
    this.reason,
    this.loading = false,
    this.error,
  });

  /// 開始時刻（「12:00」）
  final String startTime;
  final Duration remaining;
  final VoidCallback onJoin;

  /// 理由の文。null なら開始の前は「開始時刻になると入室できます」、あとは引き継ぎの文
  final String? reason;
  final bool loading;
  final String? error;

  @override
  Widget build(BuildContext context) {
    final before = remaining > Duration.zero;
    final countdown = before ? formatCountdown(remaining) : null;
    final announcement = countdownAnnouncement(remaining);
    final error = this.error;
    return Semantics(
      container: true,
      label: '入室',
      child: DecoratedBox(
        decoration: const BoxDecoration(
          color: RallyColors.background,
          border: Border(top: BorderSide(color: RallyColors.border)),
        ),
        child: SafeArea(
          top: false,
          child: Padding(
            padding: const EdgeInsets.all(RallySpacing.md),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              spacing: RallySpacing.sm,
              children: [
                // 時刻の行は 1 つの文で読み、残り時間の文は 1 分ごとと 0 になったときだけ変わる（liveRegion）
                Semantics(
                  liveRegion: true,
                  label: [
                    '開始 $startTime',
                    if (announcement.isNotEmpty) announcement,
                  ].join('、'),
                  excludeSemantics: true,
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.baseline,
                    textBaseline: TextBaseline.alphabetic,
                    spacing: RallySpacing.sm,
                    children: [
                      Text(
                        '開始',
                        style: RallyTypography.small.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      Text(startTime, style: RallyTypography.figureSm),
                      const Spacer(),
                      if (countdown != null) ...[
                        Text(
                          'あと',
                          style: RallyTypography.small.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        Text(countdown, style: RallyTypography.figureSm),
                      ],
                    ],
                  ),
                ),
                Text(
                  reason ??
                      (before
                          ? '開始時刻になると入室できます'
                          : 'カメラとマイクの入・切は、そのまま通話に引き継ぎます'),
                  style: RallyTypography.small.copyWith(
                    color: RallyColors.textMuted,
                  ),
                ),
                if (error != null)
                  Notice(message: error, status: RallyStatus.danger),
                RallyButton(
                  label: '入室する',
                  loading: loading,
                  onPressed: before ? null : onJoin,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
