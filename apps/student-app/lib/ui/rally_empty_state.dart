import 'package:flutter/material.dart';

/// 空の表示（components.yaml の empty_state）。何がないかと、どうすれば出るかを書く。
class RallyEmptyState extends StatelessWidget {
  const RallyEmptyState(this.message, {super.key});

  final String message;

  @override
  Widget build(BuildContext context) => Text(message);
}
