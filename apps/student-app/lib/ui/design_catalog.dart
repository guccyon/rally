import 'package:flutter/material.dart';

import '../api/models.dart';
import 'components/booking_card.dart';
import 'components/confirm_dialog.dart';
import 'components/description_list.dart';
import 'components/empty_state.dart';
import 'components/join_bar.dart';
import 'components/level_meter.dart';
import 'components/list_row.dart';
import 'components/media_toggle.dart';
import 'components/notice.dart';
import 'components/rally_app_bar.dart';
import 'components/rally_button.dart';
import 'components/rally_camera_preview.dart';
import 'components/rally_card.dart';
import 'components/rally_choice_chips.dart';
import 'components/score_panel.dart';
import 'components/section_loading.dart';
import 'components/status_tag.dart';
import 'components/timeline.dart';
import 'labels.dart';
import 'theme/rally_tokens.dart';

/// カタログの 1 枠。[component]・[variant]・[state] は specs/design/components.yaml の id とキー（対応はテストで確かめる）。
/// [example] は同じ部品・種類・状態の見本を見分ける名前（状態の色など。components.yaml には対応しない）。
class CatalogEntry {
  const CatalogEntry(
    this.component,
    this.builder, {
    this.variant,
    this.state,
    this.example,
  });

  final String component;
  final String? variant;
  final String? state;
  final String? example;
  final WidgetBuilder builder;

  /// 枠の名前（見出しと突き合わせに使う）。例: `button / primary / disabled`、`status_tag / warning`
  String get name => [component, ?variant, ?state, ?example].join(' / ');
}

/// components.yaml にあるが student-app のカタログに置かない部品と種類（`部品` か `部品 / 種類` → 理由）。
const notInStudentCatalog = <String, String>{
  'date_field':
      'components.yaml の not_for に student-app では今は使わないとある（admin-web の一覧の絞り込み）',
  'select': 'components.yaml の not_for に student-app では今は使わないとある（絞り込みは patterns の search_filter のチップ）',
  'data_table':
      'components.yaml の not_for にスマホの student-app とある（admin-web の一覧の表）',
  'notice / toast':
      'toast はテーマ（SnackBarThemeData）の SnackBar で出す。画面の下に重ねて出るのでカタログには置けない',
};

void _noop() {}

/// [RallyButtonVariant] → components.yaml の button.variants のキー
const _buttonVariants = {
  RallyButtonVariant.primary: 'primary',
  RallyButtonVariant.secondary: 'secondary',
  RallyButtonVariant.onDark: 'on_dark',
  RallyButtonVariant.danger: 'danger',
  RallyButtonVariant.text: 'text',
};

const _buttonLabels = {
  RallyButtonVariant.primary: '予約する',
  RallyButtonVariant.secondary: '保存する',
  RallyButtonVariant.onDark: 'コーチを探す',
  RallyButtonVariant.danger: 'キャンセルする',
  RallyButtonVariant.text: '予約をすべて見る',
};

const _notices = {
  RallyStatus.info: '11月から、コートの予約はアプリから行えます。',
  RallyStatus.success: '表示名を変更しました。',
  RallyStatus.warning: '開始まで 1 時間を切りました。',
  RallyStatus.danger: 'お知らせを読み込めませんでした。時間をおいてもう一度開いてください。',
};

/// スロットの状態の呼び名（glossary の slot.states）
const _slotStateLabels = {
  RallySlotState.open: '公開中',
  RallySlotState.booked: '予約済み',
  RallySlotState.closed: '終了',
};

const _statusLabels = {
  RallyStatus.info: 'お知らせ',
  RallyStatus.success: '確定済み',
  RallyStatus.warning: '確定待ち',
  RallyStatus.danger: '受付停止',
  RallyStatus.neutral: 'キャンセル済み',
};

/// 濃い面（inverse_surface）。on_dark の種類はこの面の中だけで使う
Widget _onDark(Widget child) => Container(
  padding: const EdgeInsets.all(RallySpacing.md),
  decoration: BoxDecoration(
    color: RallyColors.inverseSurface,
    borderRadius: BorderRadius.circular(RallyRadius.md),
  ),
  child: child,
);

Widget _button(RallyButtonVariant variant, String state) {
  final button = RallyButton(
    label: _buttonLabels[variant]!,
    variant: variant,
    loading: state == 'loading',
    onPressed: state == 'disabled' ? null : _noop,
  );
  return variant == RallyButtonVariant.onDark ? _onDark(button) : button;
}

/// 入力欄はテーマ（InputDecorationTheme）で写した TextFormField を使う
Widget _textField(String variant, String state) => TextFormField(
  initialValue: variant == 'multi_line' ? 'テニスを始めて 2 年です。' : '山田',
  enabled: state != 'disabled',
  minLines: variant == 'multi_line' ? 3 : 1,
  maxLines: variant == 'multi_line' ? null : 1,
  decoration: InputDecoration(
    labelText: variant == 'multi_line' ? '自己紹介' : '表示名',
    helperText: variant == 'multi_line' ? '500 文字まで' : '20 文字まで',
    errorText: state == 'error' ? '表示名は 1〜20 文字で入れてください。' : null,
  ),
);

/// [MediaToggleKind] → components.yaml の media_toggle.variants のキー
const _mediaToggleVariants = {
  MediaToggleKind.mic: 'mic',
  MediaToggleKind.camera: 'camera',
  MediaToggleKind.speakerTest: 'speaker_test',
};

/// 入・切の切り替えの見本。入と切の 2 つを幅を等分して並べる
Widget _mediaTogglePair(MediaToggleKind kind, String state) => Row(
  spacing: RallySpacing.sm,
  children: [
    for (final on in const [true, false])
      Expanded(
        child: MediaToggle(
          kind: kind,
          on: on,
          onToggle: _noop,
          loading: state == 'loading',
          disabled: state == 'disabled',
        ),
      ),
  ],
);

/// カメラの映りの枠の見本（components.yaml の camera_preview.states → 枠の状態と見本の名前）。
/// 映っている（default）は、カタログではカメラを開かないので地の色だけ
const _cameraPreviewExamples = [
  ('default', CameraPreviewState.live, '映像なし'),
  ('loading', CameraPreviewState.waiting, null),
  ('disabled', CameraPreviewState.off, null),
  ('error', CameraPreviewState.denied, '使えない'),
  ('error', CameraPreviewState.none, 'ない'),
];

/// 入室の帯の見本（components.yaml の join_bar.states → 開始までの残り・読み込み中・失敗）。
/// 帯は画面の下に固定するので、カタログでは枠の中に置く
const _joinBarExamples = <String, JoinBar>{
  'disabled': JoinBar(
    startTime: '12:00',
    remaining: Duration(minutes: 2, seconds: 48),
    onJoin: _noop,
  ),
  'default': JoinBar(
    startTime: '12:00',
    remaining: Duration.zero,
    onJoin: _noop,
  ),
  'loading': JoinBar(
    startTime: '12:00',
    remaining: Duration.zero,
    loading: true,
    onJoin: _noop,
  ),
  'error': JoinBar(
    startTime: '12:00',
    remaining: Duration.zero,
    error: 'この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。',
    onJoin: _noop,
  ),
};

/// ラベルと値の並びの見本（予約の詳細）。値がない項目も消さず「—」
final _bookingDetailItems = [
  const DescriptionItem('レッスンメニュー', text: 'フォアハンドの基礎（30 分）'),
  const DescriptionItem('枠の時刻', text: '10/28（水）12:00〜12:30'),
  const DescriptionItem('種類'),
  DescriptionItem(
    '今の状態',
    value: StatusTag(label: '確定済み', status: RallyBookingState.confirmed.status),
  ),
];

/// 経緯の見本（予約の状態が変わった記録。booking_state の対応の色）
final _bookingHistory = [
  TimelineEvent(
    time: '10/20（火）9:12',
    label: '確定待ち',
    status: RallyBookingState.pending.status,
  ),
  TimelineEvent(
    time: '10/21（水）8:30',
    label: '確定済み',
    status: RallyBookingState.confirmed.status,
    meta: 'コーチ: 山田',
  ),
  TimelineEvent(
    time: '10/27（火）18:04',
    label: 'キャンセル済み',
    status: RallyBookingState.cancelled.status,
    meta: '理由: コーチのお休み',
  ),
];

/// アプリバーの見本。top はホーム（入口からホームを除き、チャットに count・お知らせに dot のしるし）、back は脇の画面とフローの続き
Widget _appBar(String variant, String state) => variant == 'top'
    ? RallyAppBar(
        title: 'ホーム',
        actions: [
          for (final entry in RallyAppBarEntry.values.skip(1))
            RallyAppBarAction(
              entry,
              onPressed: _noop,
              unreadCount: entry == RallyAppBarEntry.chat ? 3 : 0,
              hasUnread: entry == RallyAppBarEntry.notifications,
            ),
        ],
      )
    : RallyAppBar(
        title: state == 'default' ? '自分の予約' : 'ログイン',
        onBack: _noop,
        busy: state == 'disabled' || state == 'loading',
        loading: state == 'loading',
      );

/// チップの選択肢（登録の「習いたいスポーツ」）。呼び名は labels.dart の語
final _sportOptions = [
  for (final sport in Sport.values) RallyChipOption(sport, sportLabel(sport)),
];

Widget _chips(String state) => RallyChoiceChips<Sport>(
  label: '習いたいスポーツ',
  options: state == 'failed' ? const [] : _sportOptions,
  selected: state == 'error' ? null : Sport.tennis,
  onSelected: (_) {},
  disabled: state == 'disabled',
  error: switch (state) {
    'error' => RallyChoiceChips.notSelectedMessage('習いたいスポーツ'),
    // 選択肢を読めないときは chip を出さず、select の load_failed の文言
    'failed' => 'スポーツの一覧を読めませんでした。画面を更新すると、もう一度読み込めます。',
    _ => null,
  },
);

/// 部品 × 種類 × 状態の見本。components.yaml に部品や種類を足したら、ここにも足す。
final designCatalog = <CatalogEntry>[
  for (final (variant, state) in const [
    ('top', 'default'),
    ('back', 'default'),
    ('back', 'disabled'),
    ('back', 'loading'),
  ])
    CatalogEntry(
      'app_bar',
      (_) => _appBar(variant, state),
      variant: variant,
      state: state,
    ),
  for (final variant in RallyButtonVariant.values)
    for (final state in const ['default', 'disabled', 'loading'])
      CatalogEntry(
        'button',
        (_) => _button(variant, state),
        variant: _buttonVariants[variant],
        state: state,
      ),
  for (final variant in const ['single_line', 'multi_line'])
    for (final state in const ['default', 'disabled', 'error'])
      CatalogEntry(
        'text_field',
        (_) => _textField(variant, state),
        variant: variant,
        state: state,
      ),
  for (final state in const ['default', 'disabled', 'error', 'failed'])
    CatalogEntry('chip', (_) => _chips(state), variant: 'choice', state: state),
  // 選択肢を読んでいるあいだは chip を出さず loading の section_text
  CatalogEntry(
    'chip',
    (_) => const SectionLoading(message: 'スポーツの一覧を読み込んでいます…'),
    variant: 'choice',
    state: 'loading',
  ),
  CatalogEntry(
    'chip',
    (_) => RallyChoiceChips<Sport>(
      label: 'スポーツ',
      options: _sportOptions,
      selected: Sport.baseball,
      onSelected: (_) {},
    ),
    variant: 'filter',
    state: 'default',
  ),
  CatalogEntry(
    'card',
    (_) => RallyCard(
      children: [
        Text('山田 花子', style: RallyTypography.title),
        Text(
          'テニス・評価 4.8（12 件）',
          style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
        ),
      ],
    ),
    variant: 'plain',
  ),
  CatalogEntry(
    'card',
    (_) => RallyCard(
      onTap: _noop,
      semanticsLabel: 'スイングの見直し、テニス、1 回 60 分',
      children: [
        Text('スイングの見直し', style: RallyTypography.title),
        Text(
          'テニス・1 回 60 分',
          style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
        ),
      ],
    ),
    variant: 'plain',
    example: '押せる',
  ),
  CatalogEntry(
    'card',
    (_) => const RallyBookingCard(
      month: '10月',
      day: '28',
      weekday: '水',
      time: '12:00〜12:30',
      meta: 'テニス・コーチ: 山田',
      status: StatusTag(label: '確定済み', status: RallyStatus.success),
      next: true,
      onTap: _noop,
      semanticsLabel: '10/28（水）12:00〜12:30、テニス・コーチ: 山田、確定済み',
    ),
    variant: 'booking',
    example: '次のレッスン',
  ),
  CatalogEntry(
    'card',
    (_) => const RallyBookingCard(
      month: '11月',
      day: '4',
      weekday: '水',
      time: '18:00〜18:30',
      meta: '野球・コーチ: 佐藤',
      status: StatusTag(label: '確定待ち', status: RallyStatus.warning),
    ),
    variant: 'booking',
    example: '確定待ち',
  ),
  CatalogEntry(
    'list_row',
    (_) => const ListRow(
      title: '10/28（水）12:00 の予約が確定しました。',
      subtitle: '2026/10/26 10:00',
      onTap: _noop,
    ),
  ),
  CatalogEntry(
    'list_row',
    (_) => const ListRow(
      title: 'コーチから返事が届きました',
      subtitle: '2026/10/27 18:04',
      unread: true,
      onTap: _noop,
    ),
    state: 'unread',
  ),
  // しるしはテーマ（BadgeThemeData）で写した Badge を使う
  CatalogEntry(
    'badge',
    (_) => Badge.count(
      count: 3,
      child: const Icon(Icons.chat_bubble_outline_rounded),
    ),
    variant: 'count',
  ),
  CatalogEntry(
    'badge',
    (_) => const Badge(child: Icon(Icons.notifications_none_rounded)),
    variant: 'dot',
  ),
  CatalogEntry(
    'badge',
    (_) => _onDark(
      Badge.count(
        count: 2,
        backgroundColor: RallyColors.secondary,
        textColor: RallyColors.onSecondary,
        child: const Icon(
          Icons.chat_bubble_outline_rounded,
          color: RallyColors.onInverseSurface,
        ),
      ),
    ),
    variant: 'on_dark',
  ),
  for (final status in RallyStatus.values)
    CatalogEntry(
      'status_tag',
      (_) => StatusTag(label: _statusLabels[status]!, status: status),
      variant: 'default',
      example: status.name,
    ),
  for (final state in RallySlotState.values)
    CatalogEntry(
      'status_tag',
      (_) => StatusTag(label: _slotStateLabels[state]!, status: state.status),
      variant: 'default',
      example: 'slot_state ${state.name}',
    ),
  CatalogEntry(
    'status_tag',
    (_) => const StatusTag(
      label: '開始まで 24 時間以内',
      status: RallyStatus.danger,
      urgent: true,
    ),
    variant: 'urgent',
  ),
  CatalogEntry(
    'score_panel',
    (_) => const ScorePanel(
      eyebrow: '今月のビデオ通話',
      prefix: '残り',
      number: '2',
      unit: '回',
      note: '月 2 回のうち（プラン: スタンダード）',
      action: RallyButton(
        label: 'スロットを探して予約する',
        variant: RallyButtonVariant.onDark,
        onPressed: _noop,
      ),
    ),
  ),
  for (final status in _notices.keys.where((s) => s != RallyStatus.success))
    CatalogEntry(
      'notice',
      (_) => Notice(message: _notices[status]!, status: status),
      variant: 'inline',
      state: status.name,
    ),
  // 成功は inline ではなく濃い面と完了の印（notice.variants.success）
  CatalogEntry(
    'notice',
    (_) => Notice(
      message: _notices[RallyStatus.success]!,
      status: RallyStatus.success,
    ),
    variant: 'success',
  ),
  CatalogEntry(
    'empty_state',
    (_) => const EmptyState(
      message: 'まだこれからの予約はありません。コーチを探して始めましょう',
      actionLabel: 'コーチを探す',
      onAction: _noop,
    ),
    variant: 'screen',
  ),
  CatalogEntry(
    'empty_state',
    (_) => const EmptyState(
      message: 'まだこれからの予約はありません。',
      actionLabel: 'コーチを探す',
      onAction: _noop,
      variant: EmptyStateVariant.section,
    ),
    variant: 'section',
  ),
  // 画面全体の読み込みはテーマ（ProgressIndicatorThemeData）で写した LinearProgressIndicator
  CatalogEntry(
    'loading',
    (_) => const LinearProgressIndicator(value: 0.3),
    variant: 'bar',
  ),
  CatalogEntry(
    'loading',
    (_) => const SectionLoading(message: 'お知らせを読み込んでいます…'),
    variant: 'section_text',
  ),
  CatalogEntry(
    'loading',
    (_) => const SizedBox.square(
      dimension: 20,
      child: CircularProgressIndicator(strokeWidth: 2, value: 0.7),
    ),
    variant: 'spinner',
  ),
  // 画面の幅 600px 未満では inline も上下に積むので、スマホのカタログでは inline と stacked が同じ形になる
  CatalogEntry(
    'description_list',
    (_) => DescriptionList(semanticsLabel: '予約の詳細', items: _bookingDetailItems),
    variant: 'inline',
    state: 'default',
  ),
  CatalogEntry(
    'description_list',
    (_) => const DescriptionList(
      variant: DescriptionListVariant.stacked,
      items: [
        DescriptionItem('コーチ', text: '山田 花子'),
        DescriptionItem(
          '本文',
          text: '11/3（火）は設備の点検のため、ビデオ通話を使えません。\n代わりの日は追ってお知らせします。',
        ),
      ],
    ),
    variant: 'stacked',
    state: 'default',
  ),
  CatalogEntry(
    'description_list',
    (_) => const DescriptionList(
      items: [
        DescriptionItem('表示名', text: '鈴木 一郎'),
        DescriptionItem('スポーツ', loading: true),
        DescriptionItem('残りの回数', error: '回数を読めませんでした。画面を更新すると、もう一度読み込めます。'),
      ],
    ),
    variant: 'inline',
    state: 'loading',
    example: '項目ごと',
  ),
  CatalogEntry(
    'description_list',
    (_) => const DescriptionList(items: [], loading: '予約を読み込んでいます…'),
    variant: 'inline',
    state: 'loading',
  ),
  CatalogEntry(
    'description_list',
    (_) => const DescriptionList(
      items: [],
      error: '予約を読み込めませんでした。画面を更新すると、もう一度読み込めます。',
    ),
    variant: 'inline',
    state: 'error',
  ),
  CatalogEntry(
    'timeline',
    (_) => Timeline(
      semanticsLabel: '状態が変わった記録',
      events: _bookingHistory,
      empty: '状態が変わった記録はありません',
    ),
    state: 'default',
  ),
  CatalogEntry(
    'timeline',
    (_) => const Timeline(
      semanticsLabel: 'お知らせの経緯',
      events: [
        TimelineEvent(time: '11/2（月）10:00', label: '送った'),
        TimelineEvent(time: '11/2（月）10:25', label: '取り下げた'),
      ],
      empty: '経緯はありません',
    ),
    state: 'default',
    example: '対応のない出来事',
  ),
  CatalogEntry(
    'timeline',
    (_) => const Timeline(
      semanticsLabel: '状態が変わった記録',
      events: [],
      empty: '状態が変わった記録はありません',
      loading: '状態が変わった記録を読み込んでいます…',
    ),
    state: 'loading',
  ),
  CatalogEntry(
    'timeline',
    (_) => const Timeline(
      semanticsLabel: '状態が変わった記録',
      events: [],
      empty: '状態が変わった記録はありません',
      error: '状態が変わった記録を読めませんでした。画面を更新すると、もう一度読み込めます。',
    ),
    state: 'error',
  ),
  CatalogEntry(
    'timeline',
    (_) => const Timeline(
      semanticsLabel: '状態が変わった記録',
      events: [],
      empty: '状態が変わった記録はありません',
    ),
    state: 'empty',
  ),
  // 確認のダイアログは開く関数（showConfirmDialog）なので、開くボタンを置く
  CatalogEntry(
    'confirm_dialog',
    (context) => RallyButton(
      label: '確認のダイアログを開く',
      variant: RallyButtonVariant.secondary,
      onPressed: () => showConfirmDialog(
        context,
        title: '予約をキャンセルしますか？',
        body: '10/28（水）12:00〜12:30 のテニスの予約をキャンセルします。今キャンセルすると、今月のビデオ通話の枠を 1 回使います。',
        confirmLabel: 'キャンセルする',
        danger: true,
      ),
    ),
  ),
  for (final kind in MediaToggleKind.values)
    for (final state in const ['default', 'disabled', 'loading'])
      CatalogEntry(
        'media_toggle',
        (_) => _mediaTogglePair(kind, state),
        variant: _mediaToggleVariants[kind],
        state: state,
      ),
  CatalogEntry(
    'media_toggle',
    (_) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      spacing: RallySpacing.sm,
      children: [
        const Notice(
          message: 'マイクを切り替えられませんでした。もう一度押してください。',
          status: RallyStatus.danger,
        ),
        _mediaTogglePair(MediaToggleKind.mic, 'default'),
      ],
    ),
    variant: 'mic',
    state: 'error',
  ),
  for (final (state, preview, example) in _cameraPreviewExamples)
    CatalogEntry(
      'camera_preview',
      (_) => RallyCameraPreview(
        state: preview,
        caption: preview == CameraPreviewState.live
            ? '映像はこの端末の中だけで映し、送りも残しもしません'
            : null,
      ),
      state: state,
      example: example,
    ),
  CatalogEntry(
    'level_meter',
    (_) => const LevelMeter(level: 6, label: '音が届いています'),
    state: 'default',
  ),
  CatalogEntry(
    'level_meter',
    (_) => const LevelMeter(level: 0, label: 'マイクの許可を待っています', stopped: true),
    state: 'loading',
  ),
  CatalogEntry(
    'level_meter',
    (_) => const LevelMeter(level: 0, label: 'マイクを切っています', stopped: true),
    state: 'disabled',
  ),
  CatalogEntry(
    'level_meter',
    (_) => const LevelMeter(
      level: 0,
      label: 'マイクが見つかりません。なくても、開始時刻から入室できます。',
      stopped: true,
    ),
    state: 'error',
  ),
  for (final MapEntry(key: state, value: bar) in _joinBarExamples.entries)
    CatalogEntry(
      'join_bar',
      (_) => DecoratedBox(
        decoration: BoxDecoration(
          border: Border.all(color: RallyColors.border),
          borderRadius: BorderRadius.circular(RallyRadius.md),
        ),
        child: Padding(
          padding: const EdgeInsets.only(top: RallySpacing.md),
          child: bar,
        ),
      ),
      state: state,
    ),
];
