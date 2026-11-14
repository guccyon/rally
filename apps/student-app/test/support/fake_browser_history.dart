import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// ブラウザーの履歴の 1 項目（アプリが報告した URL と状態）
typedef HistoryEntry = ({String uri, Object? state});

/// ブラウザーの履歴の作り物。アプリが報告した項目（`routeInformationUpdated`）を積む・置き換え、
/// [back] でブラウザーの「戻る」のように 1 つ前の項目をアプリに渡す（`pushRouteInformation`）。
/// 最初の項目はアプリを開いたページ。そこから「戻る」を押すとアプリを抜ける（[leftApp]）
class FakeBrowserHistory {
  FakeBrowserHistory(this._tester) {
    _tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.navigation,
      _handle,
    );
    addTearDown(
      () => _tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.navigation,
        null,
      ),
    );
  }

  final WidgetTester _tester;

  /// 履歴の項目（古い順）
  final List<HistoryEntry> entries = [(uri: '/', state: null)];

  /// 今の項目の位置（-1 ならアプリを抜けた）
  int index = 0;

  bool get leftApp => index < 0;

  HistoryEntry get current => entries[index];

  Future<Object?> _handle(MethodCall call) async {
    if (call.method != 'routeInformationUpdated') return null;
    final args = call.arguments as Map<Object?, Object?>;
    final entry = (uri: args['uri'] as String, state: args['state']);
    if (args['replace'] == true) {
      entries[index] = entry;
    } else {
      entries
        ..removeRange(index + 1, entries.length)
        ..add(entry);
      index++;
    }
    return null;
  }

  /// ブラウザーの「戻る」
  Future<void> back() async {
    index--;
    if (leftApp) return;
    await _go(current);
  }

  /// ブラウザーの「進む」
  Future<void> forward() async {
    index++;
    await _go(current);
  }

  Future<void> _go(HistoryEntry entry) async {
    await _tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      SystemChannels.navigation.name,
      SystemChannels.navigation.codec.encodeMethodCall(
        MethodCall('pushRouteInformation', {
          'location': entry.uri,
          'state': entry.state,
        }),
      ),
      (_) {},
    );
    await _tester.pumpAndSettle();
  }
}
