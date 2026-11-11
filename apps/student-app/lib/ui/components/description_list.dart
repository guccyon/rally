import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';
import 'notice.dart';
import 'section_loading.dart';

/// 種類（components.yaml の description_list.variants）。[inline] も画面の幅 600px 未満では上下に積む。
enum DescriptionListVariant { inline, stacked }

/// ラベルと値の 1 組。
class DescriptionItem {
  const DescriptionItem(
    this.term, {
    this.text,
    this.value,
    this.loading = false,
    this.error,
  });

  /// glossary の名詞で短く（「今の状態」）。「：」を付けない
  final String term;

  /// 文字の値。null か空なら「—」にして項目を消さない。改行は残す
  final String? text;

  /// 文字でない値（状態のタグ・リンク）。[text] より先に使う
  final Widget? value;

  /// 別の API を待つ項目だけ。値の代わりに「読み込み中…」
  final bool loading;

  /// この項目だけ読めなかったとき。値の代わりに error_text（何が起きたかとどうすればよいか）
  final String? error;
}

/// ラベルと値の並び（components.yaml の description_list。0.8.0）。
/// 1 件の項目をラベル（term 160px・small 700 text_muted）と値（body text）の組で縦に並べる。面と枠で囲わず、項目のあいだに border の 1px。
/// 項目ごとに読み上げをまとめる（MergeSemantics）。全体を読んでいるあいだは [loading]（section_text）、
/// 全体を読めなかったときは [error]（notice の inline）を渡す。
class DescriptionList extends StatelessWidget {
  const DescriptionList({
    super.key,
    required this.items,
    this.variant = DescriptionListVariant.inline,
    this.semanticsLabel,
    this.loading,
    this.error,
  });

  /// 画面の幅がこれより狭いと inline も上下に積む
  static const narrowWidth = 600.0;

  /// term の列の幅（10em）
  static const termWidth = 160.0;

  /// 値がないときの表記
  static const noValue = '—';

  final List<DescriptionItem> items;
  final DescriptionListVariant variant;

  /// 画面に 2 つ以上置くときの名前（「予約の詳細」）
  final String? semanticsLabel;
  final String? loading;
  final String? error;

  @override
  Widget build(BuildContext context) {
    if (loading case final message?) return SectionLoading(message: message);
    if (error case final message?) {
      return Notice(message: message, status: RallyStatus.danger);
    }
    final stacked =
        variant == DescriptionListVariant.stacked ||
        MediaQuery.sizeOf(context).width < narrowWidth;
    final list = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final (index, item) in items.indexed)
          _Item(item: item, stacked: stacked, first: index == 0),
      ],
    );
    return semanticsLabel == null
        ? list
        : Semantics(container: true, label: semanticsLabel, child: list);
  }
}

class _Item extends StatelessWidget {
  const _Item({required this.item, required this.stacked, required this.first});

  final DescriptionItem item;
  final bool stacked;
  final bool first;

  @override
  Widget build(BuildContext context) {
    final term = Text(
      item.term,
      style: RallyTypography.small.copyWith(
        fontWeight: FontWeight.w700,
        color: RallyColors.textMuted,
      ),
    );
    final value = _value();
    return MergeSemantics(
      child: Container(
        key: ValueKey('description-item-${item.term}'),
        padding: const EdgeInsets.symmetric(vertical: RallySpacing.sm),
        decoration: BoxDecoration(
          border: first
              ? null
              : const Border(top: BorderSide(color: RallyColors.border)),
        ),
        child: stacked
            ? Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                spacing: RallySpacing.xs,
                children: [term, value],
              )
            : Row(
                crossAxisAlignment: CrossAxisAlignment.baseline,
                textBaseline: TextBaseline.alphabetic,
                spacing: RallySpacing.md,
                children: [
                  SizedBox(width: DescriptionList.termWidth, child: term),
                  Expanded(child: value),
                ],
              ),
      ),
    );
  }

  Widget _value() {
    if (item.loading) {
      return Semantics(
        liveRegion: true,
        child: Text(
          '読み込み中…',
          style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
        ),
      );
    }
    if (item.error case final message?) {
      return Semantics(
        liveRegion: true,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          spacing: RallySpacing.xs,
          children: [
            ExcludeSemantics(
              child: Icon(
                Icons.error_outline_rounded,
                size: 20,
                color: RallyColors.danger.fg,
              ),
            ),
            Expanded(
              child: Text(
                message,
                style: RallyTypography.small.copyWith(
                  color: RallyColors.danger.fg,
                ),
              ),
            ),
          ],
        ),
      );
    }
    if (item.value case final widget?) return widget;
    final text = item.text;
    return Text(
      text == null || text.isEmpty ? DescriptionList.noValue : text,
      style: RallyTypography.body.copyWith(color: RallyColors.text),
    );
  }
}
