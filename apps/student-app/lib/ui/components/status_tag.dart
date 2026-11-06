import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 状態のタグ（components.yaml の status_tag）。8px の状態の色の点と、glossary の呼び名の文字（small 700）で出す。
/// 淡い色の面の札にしない（0.5.0）。neutral は点が border_strong で文字が text_muted。
/// [urgent] は急ぎの行（確定待ちのまま 24 時間以内に始まる予約）だけで、文字も danger.fg にする。
class StatusTag extends StatelessWidget {
  const StatusTag({
    super.key,
    required this.label,
    required this.status,
    this.urgent = false,
  });

  /// glossary の states の呼び名（「確定待ち」など）
  final String label;
  final RallyStatus status;
  final bool urgent;

  @override
  Widget build(BuildContext context) {
    final color = urgent
        ? RallyColors.danger.fg
        : status == RallyStatus.neutral
        ? RallyColors.textMuted
        : RallyColors.text;
    return Row(
      mainAxisSize: MainAxisSize.min,
      spacing: RallySpacing.xs,
      children: [
        Container(
          key: const ValueKey('status-dot'),
          width: 8,
          height: 8,
          decoration: BoxDecoration(
            color: urgent ? RallyColors.danger.fg : status.colors.fg,
            borderRadius: BorderRadius.circular(RallyRadius.pill),
          ),
        ),
        Text(
          label,
          style: RallyTypography.small.copyWith(
            fontWeight: FontWeight.w700,
            color: color,
          ),
        ),
      ],
    );
  }
}
