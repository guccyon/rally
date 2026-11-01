import 'package:flutter/material.dart';

/// 一覧の行（components.yaml の list_row）。[onTap] があれば押せる行（action）で、右端に「>」を出し、行の全体が押せる。
/// なければ表示だけの行（plain）。[enabled] が false なら無効。
class RallyListRow extends StatelessWidget {
  const RallyListRow({
    super.key,
    required this.title,
    this.subtitle,
    this.leading,
    this.onTap,
    this.enabled = true,
  });

  final String title;
  final String? subtitle;
  final Widget? leading;
  final VoidCallback? onTap;
  final bool enabled;

  @override
  Widget build(BuildContext context) => Card(
    child: ListTile(
      leading: leading,
      title: Text(title),
      subtitle: subtitle == null ? null : Text(subtitle!),
      trailing: onTap == null ? null : const Icon(Icons.chevron_right),
      enabled: enabled,
      onTap: onTap,
    ),
  );
}
