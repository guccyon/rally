import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app.dart';

void main() {
  // 同梱した数字の書体 Barlow Condensed（SIL OFL 1.1）のライセンスを、ライセンスの画面に出す
  LicenseRegistry.addLicense(() async* {
    final text = await rootBundle.loadString('assets/fonts/OFL.txt');
    yield LicenseEntryWithLineBreaks(['Barlow Condensed'], text);
  });
  runApp(const ProviderScope(child: StudentApp()));
}
