import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:yaml/yaml.dart';

/// lib/api/models.dart の enum が契約（specs/contracts/api/student-app/openapi.yaml）と同じ値を持つことを確かめる。
/// 契約を変えたらここが落ちるので、models.dart を合わせる。
void main() {
  final spec = loadYaml(
    File('../../specs/contracts/api/student-app/openapi.yaml')
        .readAsStringSync(),
  ) as YamlMap;
  final schemas = (spec['components'] as YamlMap)['schemas'] as YamlMap;

  List<String> enumOf(String schema, [String? property]) {
    var node = schemas[schema] as YamlMap;
    if (property != null) {
      node = (node['properties'] as YamlMap)[property] as YamlMap;
    }
    return (node['enum'] as YamlList).cast<String>().toList();
  }

  final cases = <String, (List<String>, List<String>)>{
    'Sport': (enumOf('Sport'), Sport.values.map((e) => e.wire).toList()),
    'BookingStatus': (
      enumOf('BookingStatus'),
      BookingStatus.values.map((e) => e.wire).toList(),
    ),
    'ContractType': (
      enumOf('ContractType'),
      ContractType.values.map((e) => e.wire).toList(),
    ),
    'SubscriptionStatus': (
      enumOf('SubscriptionStatus'),
      SubscriptionStatus.values.map((e) => e.wire).toList(),
    ),
    'LiveLessonSessionStatus': (
      enumOf('LiveLessonSessionStatus'),
      LiveLessonSessionStatus.values.map((e) => e.wire).toList(),
    ),
    'VideoStatus': (
      enumOf('VideoStatus'),
      VideoStatus.values.map((e) => e.wire).toList(),
    ),
    'SubmitVideoRequest.content_type': (
      enumOf('SubmitVideoRequest', 'content_type'),
      VideoContentType.values.map((e) => e.wire).toList(),
    ),
    'VideoReviewStatus': (
      enumOf('VideoReviewStatus'),
      VideoReviewStatus.values.map((e) => e.wire).toList(),
    ),
    'LessonType': (
      enumOf('LessonType'),
      LessonType.values.map((e) => e.wire).toList(),
    ),
    'MessageSenderRole': (
      enumOf('MessageSenderRole'),
      MessageSenderRole.values.map((e) => e.wire).toList(),
    ),
    'ErrorCode': (
      enumOf('ErrorCode'),
      ErrorCode.values
          .where((e) => e != ErrorCode.unknown)
          .map((e) => e.wire)
          .toList(),
    ),
  };

  for (final MapEntry(key: name, value: (contract, dart)) in cases.entries) {
    test('$name の値が契約と一致する', () => expect(dart, contract));
  }

  test('StudentApi が使う操作が契約にある', () {
    final operationIds = <String>{
      for (final path in (spec['paths'] as YamlMap).values)
        for (final operation in (path as YamlMap).values)
          (operation as YamlMap)['operationId'] as String,
    };
    expect(
      operationIds,
      containsAll([
        'registerAccount',
        'searchSlots',
        'createBooking',
        'confirmBooking',
        'startLiveLessonSession',
        'endLiveLessonSession',
        'submitVideo',
        'getVideoReview',
        'recordLessonCompletion',
        'submitLessonReview',
        'startSubscription',
        'listMessageThreads',
        'listMessages',
        'sendMessage',
        'markMessageThreadRead',
        'getMyNotificationSettings',
        'updateMyNotificationSettings',
      ]),
    );
  });

  List<String> propertiesOf(String schema) =>
      ((schemas[schema] as YamlMap)['properties'] as YamlMap).keys
          .cast<String>()
          .toList();

  test('NotificationKind が契約の通知の種類（NotificationSettings の真偽値の項目）と一致する', () {
    final kinds = NotificationKind.values.map((e) => e.wire).toList();
    expect(
      kinds,
      propertiesOf('NotificationSettings').where((p) => p != 'student_id'),
    );
    expect(kinds, propertiesOf('UpdateNotificationSettingsRequest'));
  });
}
