import 'package:flutter/material.dart';

import 'screens/student_flow_screen.dart';

class StudentApp extends StatelessWidget {
  const StudentApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Rally',
      theme: ThemeData(colorSchemeSeed: Colors.teal),
      home: const StudentFlowScreen(),
    );
  }
}
