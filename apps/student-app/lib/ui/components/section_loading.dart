import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 欄ごとの読み込み中（components.yaml の loading の section_text）。「〜を読み込んでいます…」。
/// 画面全体の読み込みは LinearProgressIndicator（テーマで bar の色と高さにしてある）。
class SectionLoading extends StatelessWidget {
  const SectionLoading({super.key, required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      liveRegion: true,
      child: Text(
        message,
        style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
      ),
    );
  }
}
