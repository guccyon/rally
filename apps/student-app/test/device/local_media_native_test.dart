import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/device/ios_local_media.dart';
import 'package:rally_student_app/device/local_media_native.dart';

void main() {
  tearDown(() => debugDefaultTargetPlatformOverride = null);

  test('iOS は端末の本物を使い、試し音を鳴らせる作りと言う', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;

    final media = createLocalMedia();

    expect(media, isA<IosLocalMedia>());
    expect(media.canPlayTestTone, isTrue);
  });

  test('端末の部品がない Android は、試し音を鳴らせない作りと言い、鳴らそうとすると失敗する', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.android;

    final media = createLocalMedia();

    expect(media.canPlayTestTone, isFalse);
    await expectLater(media.playTestTone(), throwsUnsupportedError);
  });
}
