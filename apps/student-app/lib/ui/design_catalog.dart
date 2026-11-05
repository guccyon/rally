import 'package:flutter/material.dart';

import 'components/confirm_dialog.dart';
import 'components/empty_state.dart';
import 'components/list_row.dart';
import 'components/notice.dart';
import 'components/rally_button.dart';
import 'components/rally_card.dart';
import 'components/score_panel.dart';
import 'components/section_loading.dart';
import 'components/status_tag.dart';
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

/// 部品 × 種類 × 状態の見本。components.yaml に部品や種類を足したら、ここにも足す。
final designCatalog = <CatalogEntry>[
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
  CatalogEntry(
    'card',
    (_) => const RallyCard(
      children: [
        Text('10/28（水）12:00〜12:30'),
        Text('テニス・コーチ: 山田'),
        StatusTag(label: '確定済み', status: RallyStatus.success),
      ],
    ),
  ),
  CatalogEntry(
    'card',
    (_) => const RallyCard(
      onTap: _noop,
      semanticsLabel: 'スイングの見直し、テニス、1 回 60 分',
      children: [Text('スイングの見直し'), Text('テニス・1 回 60 分')],
    ),
    example: '押せる',
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
      example: status.name,
    ),
  CatalogEntry(
    'score_panel',
    (_) => const ScorePanel(
      eyebrow: '今月のビデオ通話',
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
  for (final status in _notices.keys)
    CatalogEntry(
      'notice',
      (_) => Notice(message: _notices[status]!, status: status),
      variant: 'inline',
      state: status.name,
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
];
