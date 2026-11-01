import 'package:flutter/material.dart';

import 'rally_button.dart';
import 'rally_card.dart';
import 'rally_confirm_step.dart';
import 'rally_empty_state.dart';
import 'rally_input.dart';
import 'rally_list_row.dart';
import 'rally_loading.dart';
import 'rally_result_message.dart';
import 'rally_section.dart';

/// カタログの 1 枠。[component]・[variant]・[state] は specs/design/components.yaml の id（対応はテストで確かめる）。
class CatalogEntry {
  const CatalogEntry(this.component, this.variant, this.state, this.builder);

  final String component;
  final String? variant;
  final String state;
  final WidgetBuilder builder;

  /// 枠の名前（見出しと突き合わせに使う）。例: `button / primary / disabled`
  String get name => [component, ?variant, state].join(' / ');
}

void _noop() {}

const _buttonLabels = {
  RallyButtonVariant.primary: '予約を確定する',
  RallyButtonVariant.secondary: 'やめる',
  RallyButtonVariant.danger: 'キャンセルする',
};

const _sports = [('tennis', 'テニス'), ('soccer', 'サッカー')];

Widget _input(String variant, {bool enabled = true, String? errorText}) =>
    switch (variant) {
      'select' => RallySelect<String>(
        label: 'スポーツ',
        options: _sports,
        value: 'tennis',
        enabled: enabled,
        errorText: errorText,
      ),
      _ => RallyTextInput(
        label: variant == 'multi_line' ? '自己紹介' : '表示名',
        initialValue: variant == 'multi_line' ? 'テニスを始めて 2 年です。' : '山田',
        multiLine: variant == 'multi_line',
        enabled: enabled,
        errorText: errorText,
      ),
    };

/// 部品 × 種類 × 状態の全部。部品や状態を足したら、ここにも足す。
final designCatalog = <CatalogEntry>[
  for (final variant in RallyButtonVariant.values) ...[
    CatalogEntry(
      'button',
      variant.name,
      'default',
      (_) => RallyButton(
        label: _buttonLabels[variant]!,
        variant: variant,
        onPressed: _noop,
      ),
    ),
    CatalogEntry(
      'button',
      variant.name,
      'disabled',
      (_) => RallyButton(
        label: _buttonLabels[variant]!,
        variant: variant,
        onPressed: null,
      ),
    ),
    CatalogEntry(
      'button',
      variant.name,
      'busy',
      (_) => RallyButton(
        label: _buttonLabels[variant]!,
        variant: variant,
        busy: true,
        busyLabel: '送っています…',
        onPressed: _noop,
      ),
    ),
  ],
  for (final variant in const ['single_line', 'multi_line', 'select']) ...[
    CatalogEntry('input', variant, 'default', (_) => _input(variant)),
    CatalogEntry(
      'input',
      variant,
      'disabled',
      (_) => _input(variant, enabled: false),
    ),
    CatalogEntry(
      'input',
      variant,
      'invalid',
      (_) => _input(variant, errorText: '入力を確かめてください'),
    ),
  ],
  CatalogEntry(
    'card',
    null,
    'default',
    (context) => RallyCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('今月のビデオ通話', style: Theme.of(context).textTheme.titleMedium),
          const Text('残り 3 回'),
        ],
      ),
    ),
  ),
  for (final variant in const ['plain', 'action'])
    for (final state in const ['default', 'disabled'])
      CatalogEntry(
        'list_row',
        variant,
        state,
        (_) => RallyListRow(
          leading: const Icon(Icons.event_available_outlined),
          title: '10/28（水）12:00 の予約が確定しました。',
          subtitle: '2026/10/26 10:00',
          enabled: state == 'default',
          onTap: variant == 'action' ? _noop : null,
        ),
      ),
  CatalogEntry(
    'section',
    null,
    'default',
    (_) => const RallySection(title: 'お気に入りのコーチ', children: [Text('欄の中身')]),
  ),
  CatalogEntry(
    'result_message',
    'success',
    'default',
    (_) => const RallyResultMessage.success('表示名を変更しました。'),
  ),
  CatalogEntry(
    'result_message',
    'failure',
    'default',
    (_) => const RallyResultMessage.failure('お知らせを読み込めませんでした。'),
  ),
  CatalogEntry(
    'empty_state',
    null,
    'default',
    (_) => const RallyEmptyState('まだ予約はありません。スロットを探して予約すると、ここに出ます。'),
  ),
  CatalogEntry(
    'loading',
    null,
    'default',
    (_) => const RallyLoading(label: 'お知らせを読み込んでいます…'),
  ),
  for (final variant in const ['primary', 'danger'])
    for (final state in const ['default', 'busy'])
      CatalogEntry(
        'confirm_step',
        variant,
        state,
        (_) => RallyConfirmStep(
          title: '予約をキャンセルしますか？',
          message: '10/28（水）12:00〜13:00 のテニスの予約をキャンセルします。',
          items: const [('コーチ', '佐藤 コーチ'), ('メニュー', 'フォームの基本')],
          confirmLabel: 'キャンセルする',
          danger: variant == 'danger',
          busy: state == 'busy',
          busyLabel: 'キャンセルしています…',
          onConfirm: _noop,
          onCancel: _noop,
        ),
      ),
];
