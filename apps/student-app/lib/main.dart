import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app.dart';
import 'state/student_flow.dart';

void main() {
  // 同梱した数字の書体 Barlow Condensed（SIL OFL 1.1）のライセンスを、ライセンスの画面に出す
  LicenseRegistry.addLicense(() async* {
    final text = await rootBundle.loadString('assets/fonts/OFL.txt');
    yield LicenseEntryWithLineBreaks(['Barlow Condensed'], text);
  });
  final container = ProviderContainer();
  // 端末に残した登録の結果があれば、最初の画面を描く前に同じ生徒のホームにしておく（ホームの欄はこのあとサーバーから読む）
  container.read(studentFlowProvider.notifier).resumeSavedSession();
  runApp(
    UncontrolledProviderScope(container: container, child: const StudentApp()),
  );
}
