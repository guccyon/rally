import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 状態のタグ（components.yaml の status_tag）。色だけでなく、glossary の呼び名の文字とアイコンを出す。
class StatusTag extends StatelessWidget {
  const StatusTag({super.key, required this.label, required this.status});

  /// glossary の states の呼び名（「確定待ち」など）
  final String label;
  final RallyStatus status;

  @override
  Widget build(BuildContext context) {
    final colors = status.colors;
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: RallySpacing.sm,
        vertical: RallySpacing.xs,
      ),
      decoration: BoxDecoration(
        color: colors.container,
        borderRadius: BorderRadius.circular(RallyRadius.sm),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(status.icon, size: 16, color: colors.fg),
          const SizedBox(width: RallySpacing.xs),
          Text(
            label,
            style: RallyTypography.caption.copyWith(color: colors.onContainer),
          ),
        ],
      ),
    );
  }
}
