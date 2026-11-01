import 'package:flutter/material.dart';

import 'screens/student_flow_screen.dart';
import 'ui/rally_theme.dart';

class StudentApp extends StatelessWidget {
  const StudentApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Rally',
      theme: rallyTheme(),
      home: const StudentFlowScreen(),
    );
  }
}
