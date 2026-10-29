import 'package:flutter/material.dart';

/// AI の一次フィードバック（glossary の ai_feedback）の枠。コーチの添削と見分けられるよう、見出しと AI が作った文である旨を添える。
/// 添削結果の画面とチャットの添削の返却のカードで使う。
class AiFeedbackCard extends StatelessWidget {
  const AiFeedbackCard({super.key, required this.comment});

  final String comment;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      color: theme.colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('AI の一次フィードバック', style: theme.textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              'AI が自動で作った文です。コーチの添削ではありません。',
              style: theme.textTheme.bodySmall,
            ),
            const SizedBox(height: 8),
            Text(comment),
          ],
        ),
      ),
    );
  }
}
