import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import 'ios_local_media.dart';
import 'local_media.dart';

/// Web 版のほかの [LocalMedia]。iOS は端末の本物（[IosLocalMedia]）。
/// それ以外（Android の端末の部品はまだない）はカメラ・マイクはないものとして扱い、試し音は鳴らせない。
LocalMedia createLocalMedia() => defaultTargetPlatform == TargetPlatform.iOS
    ? IosLocalMedia()
    : _UnsupportedLocalMedia();

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
