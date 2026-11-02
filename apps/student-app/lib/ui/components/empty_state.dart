import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 空の表示（components.yaml の empty_state）。背中を押す一言と、次の操作（secondary か text のボタン。任意）。
class EmptyState extends StatelessWidget {
  const EmptyState({super.key, required this.message, this.action});

  final String message;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(RallySpacing.lg),
      decoration: BoxDecoration(
        color: RallyColors.surface,
        borderRadius: BorderRadius.circular(RallyRadius.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            message,
            style: RallyTypography.body.copyWith(color: RallyColors.textMuted),
          ),
          if (action != null) ...[
            const SizedBox(height: RallySpacing.md),
            action!,
          ],
        ],
      ),
    );
  }
}
