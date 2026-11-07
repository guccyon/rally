import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 予約のカード（components.yaml の card の booking）。左の日付のコマ（幅 64px・角丸 sm）に月と曜日（caption）と日（figure）、
/// 右に時刻（figure_sm）・meta（small text_muted）・状態のタグを並べる。枠は plain と同じ background に border の 1px。
/// [next] は次のレッスンだけで、コマをライム（secondary）にする。ライムのコマは 1 画面に 1 つ（principles の one-hot-spot）。
/// [onTap] があるときはカード全体を 1 つのボタンにし、日時・題・状態を 1 つの文（[semanticsLabel]）で読む。
class RallyBookingCard extends StatelessWidget {
  const RallyBookingCard({
    super.key,
    required this.month,
    required this.day,
    required this.weekday,
    required this.time,
    this.meta,
    this.status,
    this.next = false,
    this.onTap,
    this.semanticsLabel,
  });

  /// 「10月」
  final String month;

  /// 「28」
  final String day;

  /// 「水」
  final String weekday;

  /// 「12:00〜12:30」
  final String time;

  /// 「テニス・コーチ: 山田」
  final String? meta;

  /// StatusTag
  final Widget? status;
  final bool next;
  final VoidCallback? onTap;
  final String? semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final radius = BorderRadius.circular(RallyRadius.md);
    final onKoma = next ? RallyColors.onSecondary : RallyColors.text;
    final content = Padding(
      padding: const EdgeInsets.all(RallySpacing.md),
      child: IntrinsicHeight(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          spacing: RallySpacing.md,
          children: [
            Container(
              key: const ValueKey('date-block'),
              width: 64,
              padding: const EdgeInsets.symmetric(vertical: RallySpacing.sm),
              decoration: BoxDecoration(
                color: next ? RallyColors.secondary : RallyColors.surface,
                borderRadius: BorderRadius.circular(RallyRadius.sm),
              ),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    month,
                    style: RallyTypography.caption.copyWith(color: onKoma),
                  ),
                  Text(
                    day,
                    style: RallyTypography.figure.copyWith(color: onKoma),
                  ),
                  Text(
                    weekday,
                    style: RallyTypography.caption.copyWith(color: onKoma),
                  ),
                ],
              ),
            ),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                spacing: RallySpacing.xs,
                children: [
                  Text(
                    time,
                    style: RallyTypography.figureSm.copyWith(
                      color: RallyColors.text,
                    ),
                  ),
                  if (meta case final meta?)
                    Text(
                      meta,
                      style: RallyTypography.small.copyWith(
                        color: RallyColors.textMuted,
                      ),
                    ),
                  ?status,
                ],
              ),
            ),
          ],
        ),
      ),
    );
    return Material(
      color: RallyColors.background,
      shape: RoundedRectangleBorder(
        borderRadius: radius,
        side: const BorderSide(color: RallyColors.border),
      ),
      clipBehavior: Clip.antiAlias,
      child: onTap == null
          ? content
          : Semantics(
              button: true,
              label: semanticsLabel,
              excludeSemantics: semanticsLabel != null,
              child: InkWell(
                onTap: onTap,
                borderRadius: radius,
                splashFactory: NoSplash.splashFactory,
                highlightColor: RallyColors.primaryContainer,
                child: content,
              ),
            ),
    );
  }
}
