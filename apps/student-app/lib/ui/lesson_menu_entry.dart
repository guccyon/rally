import 'package:flutter/material.dart';

import '../api/models.dart';
import 'labels.dart';

/// コーチの公開中のレッスンメニュー 1 件（名前 → スポーツ・同期 / 非同期 → 同期なら 1 回の時間 → 説明）。
/// スロットを探す画面のコーチの詳細と、コーチの紹介で使う
class LessonMenuEntry extends StatelessWidget {
  const LessonMenuEntry({super.key, required this.menu});

  final LessonMenu menu;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(menu.name, style: Theme.of(context).textTheme.titleSmall),
          Text('${sportLabel(menu.sport)}・${lessonTypeLabel(menu.lessonType)}'),
          if (menu.durationMinutes case final minutes?)
            Text(lessonMenuDurationLabel(minutes)),
          if (menu.description.isNotEmpty) Text(menu.description),
        ],
      ),
    );
  }
}
