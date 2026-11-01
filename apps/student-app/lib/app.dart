import 'package:flutter/material.dart';

import 'screens/student_flow_screen.dart';

class StudentApp extends StatelessWidget {
  const StudentApp({super.key, this.home = const StudentFlowScreen()});

  /// 最初の画面。アプリは生徒の画面の流れ（[StudentFlowScreen]）で、画面のキャプチャだけがデザインシステムのカタログを渡す
  final Widget home;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Rally',
      theme: ThemeData(colorSchemeSeed: Colors.teal),
      home: home,
    );
  }
}
