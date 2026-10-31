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
    'SubscriptionPlan': (
      enumOf('SubscriptionPlan'),
      SubscriptionPlan.values.map((e) => e.wire).toList(),
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
    'MessageKind': (
      enumOf('MessageKind'),
      MessageKind.values
          .where((e) => e != MessageKind.other)
          .map((e) => e.wire)
          .toList(),
    ),
    'StampId': (
      enumOf('StampId'),
      StampId.values.map((e) => e.wire).toList(),
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
        'listMyBookings',
        'cancelBooking',
        'startLiveLessonSession',
        'endLiveLessonSession',
        'submitVideo',
        'getVideoReview',
        'recordLessonCompletion',
        'submitLessonReview',
        'getCoachReviewSummary',
        'listCoachReviews',
        'listCoachLessonMenus',
        'startSubscription',
        'getMyEntitlement',
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

  test('Slot の項目が契約と一致し、コーチの紹介は任意の項目である', () {
    // Slot.fromJson が読む項目
    expect(propertiesOf('Slot'), [
      'slot_id',
      'coach_id',
      'sport',
      'starts_at',
      'ends_at',
      'capacity',
      'remaining_seats',
      'coach_display_name',
      'coach_bio',
      'lesson_menu',
    ]);
    final required = ((schemas['Slot'] as YamlMap)['required'] as YamlList)
        .cast<String>();
    expect(required, isNot(contains('coach_display_name')));
    expect(required, isNot(contains('coach_bio')));
    expect(required, isNot(contains('lesson_menu')));
  });

  String refOf(YamlMap node) => node[r'$ref'] as String;

  List<String> requiredOf(String schema) =>
      ((schemas[schema] as YamlMap)['required'] as YamlList)
          .cast<String>()
          .toList();

  test('枠のレッスンメニューの項目が契約と一致し、メニューのない枠は null である（searchSlots）', () {
    final lessonMenu =
        ((schemas['Slot'] as YamlMap)['properties'] as YamlMap)['lesson_menu']
            as YamlMap;
    final oneOf = (lessonMenu['oneOf'] as YamlList).cast<YamlMap>().toList();
    expect(oneOf, hasLength(2));
    expect(refOf(oneOf.first), '#/components/schemas/SlotLessonMenu');
    expect(oneOf.last['type'], 'null');

    // SlotLessonMenu.fromJson が読む項目
    expect(propertiesOf('SlotLessonMenu'), [
      'lesson_menu_id',
      'name',
      'duration_minutes',
    ]);
    expect(requiredOf('SlotLessonMenu'), propertiesOf('SlotLessonMenu'));
    final duration =
        ((schemas['SlotLessonMenu'] as YamlMap)['properties']
                as YamlMap)['duration_minutes']
            as YamlMap;
    expect((duration['type'] as YamlList).cast<String>(), ['integer', 'null']);
  });

  test('予約のレッスンメニューの項目が契約と一致し、メニューのない予約は null である（listMyBookings）', () {
    // Booking.fromJson が読む項目
    expect(propertiesOf('Booking'), [
      'booking_id',
      'slot_id',
      'student_id',
      'coach_id',
      'is_trial',
      'status',
      'starts_at',
      'created_at',
      'confirmed_at',
      'cancelled_at',
      'lesson_menu',
    ]);
    final lessonMenu =
        ((schemas['Booking'] as YamlMap)['properties']
                as YamlMap)['lesson_menu']
            as YamlMap;
    final oneOf = (lessonMenu['oneOf'] as YamlList).cast<YamlMap>().toList();
    expect(oneOf, hasLength(2));
    expect(refOf(oneOf.first), '#/components/schemas/BookingLessonMenu');
    expect(oneOf.last['type'], 'null');

    // BookingLessonMenu.fromJson が読む項目
    expect(propertiesOf('BookingLessonMenu'), [
      'lesson_menu_id',
      'name',
      'duration_minutes',
    ]);
    expect(requiredOf('BookingLessonMenu'), propertiesOf('BookingLessonMenu'));
    final duration =
        ((schemas['BookingLessonMenu'] as YamlMap)['properties']
                as YamlMap)['duration_minutes']
            as YamlMap;
    expect((duration['type'] as YamlList).cast<String>(), ['integer', 'null']);
  });

  test('契約プランの項目が契約と一致する（startSubscription・getMyEntitlement）', () {
    // StartSubscriptionRequest.toJson が書く項目。plan は任意（省けば standard）
    expect(propertiesOf('StartSubscriptionRequest'), [
      'sport',
      'plan',
      'payment_method_token',
    ]);
    expect(requiredOf('StartSubscriptionRequest'), isNot(contains('plan')));
    final requestPlan =
        ((schemas['StartSubscriptionRequest'] as YamlMap)['properties']
                as YamlMap)['plan']
            as YamlMap;
    expect(refOf(requestPlan), '#/components/schemas/SubscriptionPlan');

    // Subscription.fromJson・Entitlement.fromJson が読む項目
    expect(propertiesOf('Subscription'), contains('plan'));
    expect(requiredOf('Subscription'), contains('plan'));
    expect(propertiesOf('Entitlement'), contains('plan'));
    expect(requiredOf('Entitlement'), isNot(contains('plan')));
    expect(propertiesOf('LiveLessonQuota'), [
      'month',
      'limit',
      'used',
      'remaining',
    ]);
  });

  test('コーチの評価の項目が契約と一致し、評価した生徒を載せない（getCoachReviewSummary・listCoachReviews）', () {
    // CoachReviewSummary.fromJson・CoachReview.fromJson・CoachReviewList.fromJson が読む項目
    expect(propertiesOf('CoachReviewSummary'), [
      'coach_id',
      'review_count',
      'average_rating',
    ]);
    final average =
        ((schemas['CoachReviewSummary'] as YamlMap)['properties']
                as YamlMap)['average_rating']
            as YamlMap;
    expect((average['type'] as YamlList).cast<String>(), ['number', 'null']);
    expect(propertiesOf('CoachReview'), [
      'review_id',
      'lesson_id',
      'rating',
      'comment',
      'submitted_at',
    ]);
    expect(propertiesOf('CoachReview'), isNot(contains('student_id')));
    expect(propertiesOf('CoachReviewList'), ['coach_id', 'reviews']);
    final items =
        (((schemas['CoachReviewList'] as YamlMap)['properties']
                    as YamlMap)['reviews']
                as YamlMap)['items']
            as YamlMap;
    expect(refOf(items), '#/components/schemas/CoachReview');

    for (final (path, operationId, schema) in [
      (
        '/coaches/{coach_id}/review-summary',
        'getCoachReviewSummary',
        'CoachReviewSummary',
      ),
      ('/coaches/{coach_id}/reviews', 'listCoachReviews', 'CoachReviewList'),
    ]) {
      final operation =
          ((spec['paths'] as YamlMap)[path] as YamlMap)['get'] as YamlMap;
      expect(operation['operationId'], operationId);
      final ok = ((operation['responses'] as YamlMap)['200'] as YamlMap);
      final body =
          (((ok['content'] as YamlMap)['application/json'] as YamlMap)['schema']
              as YamlMap);
      expect(refOf(body), '#/components/schemas/$schema');
    }
  });

  test('レッスンメニューの項目が契約と一致する（listCoachLessonMenus）', () {
    // LessonMenu.fromJson・LessonMenuList.fromJson が読む項目
    expect(propertiesOf('LessonMenu'), [
      'lesson_menu_id',
      'coach_id',
      'sport',
      'lesson_type',
      'name',
      'description',
      'duration_minutes',
      'is_published',
      'created_at',
    ]);
    expect(requiredOf('LessonMenu'), propertiesOf('LessonMenu'));
    final properties =
        (schemas['LessonMenu'] as YamlMap)['properties'] as YamlMap;
    expect(refOf(properties['sport'] as YamlMap), '#/components/schemas/Sport');
    expect(
      refOf(properties['lesson_type'] as YamlMap),
      '#/components/schemas/LessonType',
    );
    expect(
      ((properties['duration_minutes'] as YamlMap)['type'] as YamlList)
          .cast<String>(),
      ['integer', 'null'],
    );
    expect(propertiesOf('LessonMenuList'), ['coach_id', 'lesson_menus']);
    final items =
        (((schemas['LessonMenuList'] as YamlMap)['properties']
                    as YamlMap)['lesson_menus']
                as YamlMap)['items']
            as YamlMap;
    expect(refOf(items), '#/components/schemas/LessonMenu');

    final operation =
        ((spec['paths'] as YamlMap)['/coaches/{coach_id}/lesson-menus']
                as YamlMap)['get']
            as YamlMap;
    expect(operation['operationId'], 'listCoachLessonMenus');
    final sport = (operation['parameters'] as YamlList)
        .cast<YamlMap>()
        .firstWhere((p) => p['name'] == 'sport');
    expect(sport['in'], 'query');
    expect(sport['required'], false);
    expect(refOf(sport['schema'] as YamlMap), '#/components/schemas/Sport');
  });

  test('Message の項目が契約と一致し、種類ごとの中身は任意の項目である（listMessages・sendMessage）', () {
    // Message.fromJson が読む項目
    expect(propertiesOf('Message'), [
      'message_id',
      'thread_id',
      'sender_id',
      'sender_role',
      'kind',
      'body',
      'video_id',
      'review_id',
      'stamp_id',
      'sent_at',
      'read_at',
    ]);
    expect(requiredOf('Message'), containsAll(['kind', 'body']));
    expect(requiredOf('Message'), isNot(contains('video_id')));
    expect(requiredOf('Message'), isNot(contains('review_id')));
    expect(requiredOf('Message'), isNot(contains('stamp_id')));
    final properties =
        (schemas['Message'] as YamlMap)['properties'] as YamlMap;
    expect(
      refOf(properties['kind'] as YamlMap),
      '#/components/schemas/MessageKind',
    );
    expect(
      refOf(properties['stamp_id'] as YamlMap),
      '#/components/schemas/StampId',
    );
  });

  test('SendMessageRequest は本文かスタンプのどちらかで、どちらも任意の項目である（sendMessage）', () {
    // SendMessageRequest.toJson が書く項目
    expect(propertiesOf('SendMessageRequest'), ['body', 'stamp_id']);
    expect((schemas['SendMessageRequest'] as YamlMap)['required'], isNull);
    final properties =
        (schemas['SendMessageRequest'] as YamlMap)['properties'] as YamlMap;
    expect(
      (properties['body'] as YamlMap)['maxLength'],
      SendMessageRequest.maxBodyLength,
    );
    expect(
      refOf(properties['stamp_id'] as YamlMap),
      '#/components/schemas/StampId',
    );
  });

  test('MyBooking・MyBookingList の項目が契約と一致する（listMyBookings の応答）', () {
    // MyBooking.fromJson・MyBookingList.fromJson が読む項目
    expect(propertiesOf('MyBooking'), ['booking', 'slot']);
    final myBooking =
        (schemas['MyBooking'] as YamlMap)['properties'] as YamlMap;
    expect(
      refOf(myBooking['booking'] as YamlMap),
      '#/components/schemas/Booking',
    );
    expect(refOf(myBooking['slot'] as YamlMap), '#/components/schemas/Slot');
    expect(propertiesOf('MyBookingList'), ['bookings']);
    final items =
        (((schemas['MyBookingList'] as YamlMap)['properties']
                    as YamlMap)['bookings']
                as YamlMap)['items']
            as YamlMap;
    expect(refOf(items), '#/components/schemas/MyBooking');

    final operation =
        ((spec['paths'] as YamlMap)['/bookings/me'] as YamlMap)['get']
            as YamlMap;
    expect(operation['operationId'], 'listMyBookings');
    final ok = ((operation['responses'] as YamlMap)['200'] as YamlMap);
    final schema =
        (((ok['content'] as YamlMap)['application/json'] as YamlMap)['schema']
            as YamlMap);
    expect(refOf(schema), '#/components/schemas/MyBookingList');
  });
}
