import 'package:flutter/material.dart';

/// カード（components.yaml の card）。ひとまとまりの情報を枠で囲む。
class RallyCard extends StatelessWidget {
  const RallyCard({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => Card(
    child: Padding(padding: const EdgeInsets.all(12), child: child),
  );
}
