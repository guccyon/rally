import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'router/student_route_delegate.dart';
import 'screens/student_flow_screen.dart';
import 'ui/theme/rally_theme.dart';

/// 生徒アプリ。段の移動をブラウザーの履歴に写し（[StudentRouterDelegate]）、「戻る」でアプリから抜けないようにする
class StudentApp extends StatefulWidget {
  const StudentApp({super.key, this.home = const StudentFlowScreen()});

  /// 最初の画面。アプリは生徒の画面の流れ（[StudentFlowScreen]）で、画面のキャプチャだけがデザインシステムのカタログを渡す
  final Widget home;

  @override
  State<StudentApp> createState() => _StudentAppState();
}

class _StudentAppState extends State<StudentApp> {
  late final _routerDelegate = StudentRouterDelegate(
    ProviderScope.containerOf(context, listen: false),
    home: widget.home,
  );
  final _routeInformationProvider = StudentRouteInformationProvider();

  @override
  void dispose() {
    _routerDelegate.dispose();
    _routeInformationProvider.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'Rally',
      theme: buildRallyTheme(),
      // 明るい配色だけ（specs/design/tokens.yaml の color.dark_mode.gen1）
      themeMode: ThemeMode.light,
      routerDelegate: _routerDelegate,
      routeInformationParser: const StudentRouteInformationParser(),
      routeInformationProvider: _routeInformationProvider,
    );
  }
}
