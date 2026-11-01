import 'package:flutter/material.dart';

/// セクション（components.yaml の section）。画面の中の欄で、見出しを付ける。
class RallySection extends StatelessWidget {
  const RallySection({super.key, required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: Semantics(
            header: true,
            child: Text(title, style: Theme.of(context).textTheme.titleMedium),
          ),
        ),
        ...children,
      ],
    ),
  );
}
