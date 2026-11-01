import 'package:flutter/material.dart';

/// 読み込み中（components.yaml の loading）。「〜を読み込んでいます…」の 1 行。
class RallyLoading extends StatelessWidget {
  const RallyLoading({super.key, this.label = '読み込んでいます…'});

  final String label;

  @override
  Widget build(BuildContext context) => Text(label);
}
