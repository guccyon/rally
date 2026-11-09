import 'package:flutter/widgets.dart';

import 'local_media.dart';

/// Web 版のほか（iOS / Android のビルドはまだない）の [LocalMedia]。カメラ・マイクはないものとして扱い、試し音は鳴らせない。
LocalMedia createLocalMedia() => _UnsupportedLocalMedia();

class _UnsupportedLocalMedia implements LocalMedia {
  @override
  Future<MediaDeviceStatus> openCamera() async => MediaDeviceStatus.missing;

  @override
  Future<MediaDeviceStatus> openMic() async => MediaDeviceStatus.missing;

  @override
  void stopCamera() {}

  @override
  void stopMic() {}

  @override
  Widget? cameraView() => null;

  @override
  Stream<double> get micLevels => const Stream.empty();

  @override
  Future<void> playTestTone() =>
      Future.error(UnsupportedError('この端末では試し音を鳴らせません'));

  @override
  void stopTestTone() {}

  @override
  bool get canOpenSettings => false;

  @override
  Future<void> openSettings() async {}

  @override
  void stopAll() {}
}
