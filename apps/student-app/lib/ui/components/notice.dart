import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 知らせの inline（components.yaml の notice）。欄の中か画面の上に出し、自動で消さない。
/// 読み上げには liveRegion で伝える。失敗は「何が起きたか」と「どうすればよいか」の 2 文で書く。
class Notice extends StatelessWidget {
  const Notice({super.key, required this.message, required this.status});

  final String message;
  final RallyStatus status;

  @override
  Widget build(BuildContext context) {
    final colors = status.colors;
    return Semantics(
      liveRegion: true,
      child: Container(
        padding: const EdgeInsets.all(RallySpacing.md),
        decoration: BoxDecoration(
          color: colors.container,
          borderRadius: BorderRadius.circular(RallyRadius.md),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(status.icon, color: colors.fg),
            const SizedBox(width: RallySpacing.sm),
            Expanded(
              child: Text(
                message,
                style: RallyTypography.body.copyWith(color: colors.onContainer),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
