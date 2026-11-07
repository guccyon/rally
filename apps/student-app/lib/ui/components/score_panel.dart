import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// スコアの面（components.yaml の score_panel）。画面の主役の数字を 1 つ見せる。1 画面に 1 つまで。
/// 数字は display（numeric の Barlow Condensed 64px 800）でライムの 4px の下線、単位は title で添える。
/// 読み上げでは eyebrow・数字と単位・note を 1 つの文で読む（「今月のビデオ通話、残り 2 回、月 2 回のうち」）。
class ScorePanel extends StatelessWidget {
  const ScorePanel({
    super.key,
    required this.eyebrow,
    required this.number,
    this.prefix,
    this.unit,
    this.note,
    this.action,
  });

  final String eyebrow;
  final String number;

  /// 数字の前の small の言葉（「残り」）
  final String? prefix;
  final String? unit;

  /// 数字の条件（「月 2 回のうち」）。本文と同じ言葉で書く
  final String? note;

  /// on_dark のボタン（任意）
  final Widget? action;

  String get semanticsLabel => [
    eyebrow,
    [?prefix, number, ?unit].join(' '),
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
                Wrap(
                  crossAxisAlignment: WrapCrossAlignment.end,
                  spacing: RallySpacing.sm,
                  children: [
                    if (prefix case final prefix?)
                      Text(
                        prefix,
                        style: RallyTypography.small.copyWith(
                          color: RallyColors.onInverseSurface,
                        ),
                      ),
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
                      child: Text(
                        number,
                        style: RallyTypography.display.copyWith(
                          color: RallyColors.onInverseSurface,
                        ),
                      ),
                    ),
                    if (unit case final unit?)
                      Padding(
                        padding: const EdgeInsets.only(bottom: RallySpacing.sm),
                        child: Text(
                          unit,
                          style: RallyTypography.title.copyWith(
                            color: RallyColors.onInverseSurface,
                          ),
                        ),
                      ),
                  ],
                ),
                if (note != null) ...[
                  const SizedBox(height: RallySpacing.sm),
                  Text(
                    note!,
                    style: RallyTypography.small.copyWith(
                      color: RallyColors.onInverseSurfaceMuted,
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
