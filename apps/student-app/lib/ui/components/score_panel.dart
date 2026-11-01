import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// スコアの面（components.yaml の score_panel）。画面の主役の数字を 1 つ見せる。1 画面に 1 つまで。
/// 読み上げでは eyebrow・数字と単位・note を 1 つの文で読む（「今月のビデオ通話、残り 2 回、月 2 回のうち」）。
class ScorePanel extends StatelessWidget {
  const ScorePanel({
    super.key,
    required this.eyebrow,
    required this.number,
    this.unit,
    this.note,
    this.action,
  });

  final String eyebrow;
  final String number;
  final String? unit;

  /// 数字の条件（「月 2 回のうち」）。本文と同じ言葉で書く
  final String? note;

  /// on_dark のボタン（任意）
  final Widget? action;

  String get semanticsLabel => [
    eyebrow,
    [number, ?unit].join(' '),
    ?note,
  ].join('、');

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(RallySpacing.lg),
      decoration: BoxDecoration(
        color: RallyColors.inverseSurface,
        borderRadius: BorderRadius.circular(RallyRadius.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Semantics(
            label: semanticsLabel,
            excludeSemantics: true,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  eyebrow,
                  style: RallyTypography.small.copyWith(
                    color: RallyColors.onInverseSurfaceMuted,
                  ),
                ),
                const SizedBox(height: RallySpacing.sm),
                Container(
                  padding: const EdgeInsets.only(bottom: RallySpacing.xs),
                  decoration: const BoxDecoration(
                    border: Border(
                      bottom: BorderSide(
                        color: RallyColors.secondary,
                        width: 4,
                      ),
                    ),
                  ),
                  child: Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(text: number, style: RallyTypography.display),
                        if (unit != null)
                          TextSpan(
                            text: ' $unit',
                            style: RallyTypography.title,
                          ),
                      ],
                    ),
                    style: const TextStyle(color: RallyColors.onInverseSurface),
                  ),
                ),
                if (note != null) ...[
                  const SizedBox(height: RallySpacing.sm),
                  Text(
                    note!,
                    style: RallyTypography.small.copyWith(
                      color: RallyColors.onInverseSurface,
                    ),
                  ),
                ],
              ],
            ),
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
