import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';
import 'rally_button.dart';

/// 空の表示の種類（components.yaml の empty_state.variants）。
enum EmptyStateVariant {
  /// 画面にほかの主な操作（primary）がない。surface の面と primary のボタン
  screen,

  /// 欄だけが空で、画面にほかの primary がある。background の面と border の線と secondary のボタン
  section,
}

/// 空の表示（components.yaml の empty_state）。背中を押す一言と、次の操作（任意）。
/// surface の面に secondary のボタンを置かないよう、ボタンの種類は [variant] で決める。
class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.message,
    this.actionLabel,
    this.onAction,
    this.variant = EmptyStateVariant.screen,
  });

  final String message;

  /// 次の操作の文言（動詞で終える）。null ならボタンを出さない
  final String? actionLabel;
  final VoidCallback? onAction;
  final EmptyStateVariant variant;

  @override
  Widget build(BuildContext context) {
    final screen = variant == EmptyStateVariant.screen;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(RallySpacing.lg),
      decoration: BoxDecoration(
        color: screen ? RallyColors.surface : RallyColors.background,
        border: screen ? null : Border.all(color: RallyColors.border),
        borderRadius: BorderRadius.circular(RallyRadius.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            message,
            style: RallyTypography.body.copyWith(color: RallyColors.text),
          ),
          if (actionLabel case final label?) ...[
            const SizedBox(height: RallySpacing.md),
            RallyButton(
              label: label,
              onPressed: onAction,
              variant: screen
                  ? RallyButtonVariant.primary
                  : RallyButtonVariant.secondary,
            ),
          ],
        ],
      ),
    );
  }
}
