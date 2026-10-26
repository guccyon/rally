import 'package:flutter/material.dart';

import '../api/models.dart';
import 'labels.dart';

/// スポーツを 1 つ選ぶチップの列（ゴルフは student-app の範囲外なので出ない）。
class SportSelector extends StatelessWidget {
  const SportSelector({
    super.key,
    required this.selected,
    required this.onSelected,
  });

  final Sport? selected;
  final ValueChanged<Sport> onSelected;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      children: [
        for (final sport in Sport.values)
          ChoiceChip(
            label: Text(sportLabel(sport)),
            selected: sport == selected,
            onSelected: (_) => onSelected(sport),
          ),
      ],
    );
  }
}
