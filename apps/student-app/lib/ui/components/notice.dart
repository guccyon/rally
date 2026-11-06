import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 知らせ（components.yaml の notice）。自動で消さない。読み上げには liveRegion で伝える。
/// 失敗・注意・情報は inline（background に border の 1px と、左に状態の色の 4px の線とアイコン。文字は text）。
/// 成功は success（inverse_surface の濃い面と、完了の印のライムの丸）。淡い色の面は使わない（0.5.0）。
/// 失敗は「何が起きたか」と「どうすればよいか」の 2 文で書く。
class Notice extends StatelessWidget {
  const Notice({super.key, required this.message, required this.status});

  final String message;
  final RallyStatus status;

  @override
  Widget build(BuildContext context) {
    final success = status == RallyStatus.success;
    final radius = BorderRadius.circular(RallyRadius.md);
    final body = Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      spacing: RallySpacing.sm,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: RallySpacing.xs),
          child: success
              ? const _DoneMark()
              : ExcludeSemantics(
                  child: Icon(status.icon, size: 20, color: status.colors.fg),
                ),
        ),
        Expanded(
          child: Text(
            message,
            style: success
                ? RallyTypography.body.copyWith(
                    fontWeight: FontWeight.w700,
                    color: RallyColors.onInverseSurface,
                  )
                : RallyTypography.body.copyWith(color: RallyColors.text),
          ),
        ),
      ],
    );
    return Semantics(
      liveRegion: true,
      child: success
          ? Container(
              padding: const EdgeInsets.all(RallySpacing.md),
              decoration: BoxDecoration(
                color: RallyColors.inverseSurface,
                borderRadius: radius,
              ),
              child: body,
            )
          : ClipRRect(
              borderRadius: radius,
              child: Container(
                padding: const EdgeInsets.all(RallySpacing.md),
                decoration: BoxDecoration(
                  color: RallyColors.background,
                  border: Border(
                    left: BorderSide(color: status.colors.fg, width: 4),
                    top: const BorderSide(color: RallyColors.border),
                    right: const BorderSide(color: RallyColors.border),
                    bottom: const BorderSide(color: RallyColors.border),
                  ),
                ),
                child: body,
              ),
            ),
    );
  }
}

/// 完了の印（iconography.status_mark）。20px のライムの丸にインクのチェック。成功の知らせの中だけで、読み上げない
class _DoneMark extends StatelessWidget {
  const _DoneMark();

  @override
  Widget build(BuildContext context) {
    return ExcludeSemantics(
      child: Container(
        key: const ValueKey('done-mark'),
        width: 20,
        height: 20,
        decoration: BoxDecoration(
          color: RallyColors.secondary,
          borderRadius: BorderRadius.circular(RallyRadius.pill),
        ),
        child: const Icon(
          Icons.check_rounded,
          size: 16,
          color: RallyColors.onSecondary,
        ),
      ),
    );
  }
}
