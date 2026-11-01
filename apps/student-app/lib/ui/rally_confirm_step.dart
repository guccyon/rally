import 'package:flutter/material.dart';

import 'rally_button.dart';

/// 確認の段（components.yaml の confirm_step）。取り消せない操作・相手に届く操作の前に、内容を見せて確定かやめるを選ぶ。
/// [danger] なら確定のボタンを危険（danger）にする。[busy] の間は両方押せない。
/// ダイアログ（showDialog の builder）としても、画面の中にそのままでも置ける。
class RallyConfirmStep extends StatelessWidget {
  const RallyConfirmStep({
    super.key,
    required this.title,
    required this.message,
    required this.confirmLabel,
    required this.onConfirm,
    required this.onCancel,
    this.items = const [],
    this.cancelLabel = 'やめる',
    this.danger = false,
    this.busy = false,
    this.busyLabel,
  });

  final String title;

  /// 何が起きるか（誰に届くか・取り消せないこと）の 1 文
  final String message;

  /// 内容の要約（見出しと値の組）
  final List<(String, String)> items;
  final String confirmLabel;
  final String cancelLabel;
  final VoidCallback onConfirm;
  final VoidCallback onCancel;
  final bool danger;
  final bool busy;
  final String? busyLabel;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return AlertDialog(
      title: Text(title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(message),
          for (final (term, detail) in items) ...[
            const SizedBox(height: 8),
            Text(term, style: textTheme.labelMedium),
            Text(detail),
          ],
        ],
      ),
      actions: [
        RallyButton(
          label: cancelLabel,
          variant: RallyButtonVariant.secondary,
          onPressed: busy ? null : onCancel,
        ),
        RallyButton(
          label: confirmLabel,
          variant: danger
              ? RallyButtonVariant.danger
              : RallyButtonVariant.primary,
          busy: busy,
          busyLabel: busyLabel,
          onPressed: onConfirm,
        ),
      ],
    );
  }
}
