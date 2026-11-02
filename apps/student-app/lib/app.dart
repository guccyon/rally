import 'package:flutter/material.dart';

import 'screens/student_flow_screen.dart';
import 'ui/theme/rally_theme.dart';

class StudentApp extends StatelessWidget {
  const StudentApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Rally',
      theme: buildRallyTheme(),
      // 明るい配色だけ（specs/design/tokens.yaml の color.dark_mode.gen1）
      themeMode: ThemeMode.light,
      home: const StudentFlowScreen(),
    );
  }
}
