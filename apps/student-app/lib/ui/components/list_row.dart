import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 一覧の行（components.yaml の list_row）。押すと詳細へ移る。
/// 未読は左に primary の 4px の線と、題の前の「未読」のしるしで示し、読み上げでも「未読」と読む。
class ListRow extends StatelessWidget {
  const ListRow({
    super.key,
    required this.title,
    this.subtitle,
    this.leading,
    this.trailing,
    this.unread = false,
    this.onTap,
  });

  final String title;
  final String? subtitle;
  final Widget? leading;
  final Widget? trailing;
  final bool unread;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          left: unread
              ? const BorderSide(color: RallyColors.primary, width: 4)
              : BorderSide.none,
          bottom: const BorderSide(color: RallyColors.border),
        ),
      ),
      child: ListTile(
        leading: leading,
        title: Row(
          children: [
            if (unread) ...[
              const _UnreadMark(),
              const SizedBox(width: RallySpacing.sm),
            ],
            Flexible(child: Text(title)),
          ],
        ),
        subtitle: subtitle == null ? null : Text(subtitle!),
        trailing:
            trailing ??
            (onTap == null ? null : const Icon(Icons.chevron_right_rounded)),
        onTap: onTap,
        splashColor: Colors.transparent,
        hoverColor: RallyColors.primaryContainer,
        focusColor: RallyColors.primaryContainer,
      ),
    );
  }
}

class _UnreadMark extends StatelessWidget {
  const _UnreadMark();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: RallySpacing.xs),
      decoration: BoxDecoration(
        color: RallyColors.primary,
        borderRadius: BorderRadius.circular(RallyRadius.sm),
      ),
      child: Text(
        '未読',
        style: RallyTypography.caption.copyWith(color: RallyColors.onPrimary),
      ),
    );
  }
}
