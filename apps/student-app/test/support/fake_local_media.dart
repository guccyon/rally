import 'dart:async';

import 'package:flutter/material.dart';
import 'package:rally_student_app/device/local_media.dart';

/// テスト用の [LocalMedia]（作り物の端末）。開いた結果を [camera] と [mic] で決め、操作を [calls] に順に残す。
/// 音の大きさは [emitLevel] で流し、試し音は [finishTone] で鳴り終わらせる。
class FakeLocalMedia implements LocalMedia {
  FakeLocalMedia({
    this.camera = MediaDeviceStatus.ready,
    this.mic = MediaDeviceStatus.ready,
    this.toneFails = false,
    this.canOpenSettings = false,
  });

  /// [openCamera] が返す結果（「もう一度確かめる」の前に変えられる）
  MediaDeviceStatus camera;

  /// [openMic] が返す結果
  MediaDeviceStatus mic;

  /// 試し音を鳴らせない端末
  bool toneFails;

  @override
  bool canOpenSettings;

  /// 入れると、[openCamera] はこれが終わるまで応答しない（許可を待っている表示を見る）
  Completer<void>? permissionGate;

  final calls = <String>[];

  bool cameraOpen = false;
  bool micOpen = false;
  bool tonePlaying = false;

  final _levels = StreamController<double>.broadcast(sync: true);
  Completer<void>? _tone;

  @override
  Future<MediaDeviceStatus> openCamera() async {
    calls.add('openCamera');
    await permissionGate?.future;
    cameraOpen = camera == MediaDeviceStatus.ready;
    return camera;
  }

  @override
  Future<MediaDeviceStatus> openMic() async {
    calls.add('openMic');
    micOpen = mic == MediaDeviceStatus.ready;
    return mic;
  }

  @override
  void stopCamera() {
    calls.add('stopCamera');
    cameraOpen = false;
  }

  @override
  void stopMic() {
    calls.add('stopMic');
    micOpen = false;
  }

  /// 本物の映像の代わりの枠（キャプチャでは人の形の図）
  @override
  Widget? cameraView() => cameraOpen
      ? const SizedBox(
          key: ValueKey('fake-camera'),
          width: 640,
          height: 480,
          child: ColoredBox(
            color: Color(0xFF55606B),
            child: Icon(
              Icons.person_rounded,
              size: 320,
              color: Color(0xFFB9BDB3),
            ),
          ),
        )
      : null;

  @override
  Stream<double> get micLevels => _levels.stream;

  /// マイクの音の大きさを 1 回流す（0.0〜1.0）
  void emitLevel(double level) => _levels.add(level);

  @override
  Future<void> playTestTone() {
    calls.add('playTestTone');
    if (toneFails) return Future.error(StateError('出力がない'));
    tonePlaying = true;
    return (_tone = Completer<void>()).future;
  }

  /// 鳴っている試し音を鳴り終わらせる
  void finishTone() {
    tonePlaying = false;
    _tone?.complete();
    _tone = null;
  }

  @override
  void stopTestTone() {
    calls.add('stopTestTone');
    finishTone();
  }

  @override
  Future<void> openSettings() async => calls.add('openSettings');

  @override
  void stopAll() {
    calls.add('stopAll');
    cameraOpen = false;
    micOpen = false;
    finishTone();
  }
}
