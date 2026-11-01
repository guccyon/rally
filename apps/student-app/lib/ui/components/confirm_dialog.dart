import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';
import 'rally_button.dart';

/// 確認のダイアログ（components.yaml の confirm_dialog）を開き、確定したら true を返す。
/// 題は疑問の文、本文に対象と結果と条件を省かずに書く。cancel（「やめる」）を左、confirm を右に置き、
/// 外を押す・戻るは cancel と同じ。[danger] は取り消せない操作の確認。
Future<bool> showConfirmDialog(
  BuildContext context, {
  required String title,
  required String body,
  required String confirmLabel,
  String cancelLabel = 'やめる',
  bool danger = false,
}) async {
  final confirmed = await showDialog<bool>(
    context: context,
    barrierColor: RallyColors.scrim,
    builder: (context) => AlertDialog(
      constraints: const BoxConstraints(maxWidth: 400),
      contentPadding: const EdgeInsets.fromLTRB(
        RallySpacing.lg,
        RallySpacing.md,
        RallySpacing.lg,
        RallySpacing.lg,
      ),
      title: Text(title),
      content: Text(body),
      actions: [
        RallyButton(
          label: cancelLabel,
          variant: RallyButtonVariant.text,
          onPressed: () => Navigator.of(context).pop(false),
        ),
        RallyButton(
          label: confirmLabel,
          variant: danger
              ? RallyButtonVariant.danger
              : RallyButtonVariant.primary,
          onPressed: () => Navigator.of(context).pop(true),
        ),
      ],
    ),
  );
  return confirmed ?? false;
}
