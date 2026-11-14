import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// チップの選択肢 1 つ。[label] は glossary と labels.dart の語（「テニス」）。
class RallyChipOption<T> {
  const RallyChipOption(this.value, this.label);

  final T value;
  final String label;
}

/// チップ（選択。components.yaml の chip。0.10.0）。並ぶ 2〜6 個から 1 つを選ぶ。
/// 種類は choice（必須の 1 つ。初めは何も選ばず、送るときに確かめる）と filter（選んだらすぐ読み直す）で、見た目は同じ。
/// 選んでいない chip は background・text、選んだ chip は primary_container・small 700 と前に check を出し、読み上げは radio の「選択中」。
/// 押したらその場で [onSelected] を呼ぶ。選んだ chip をもう一度押しても外さない。
/// [disabled] は送信中だけで、全部を surface・text_muted・輪郭なしにする（選んだ chip の check と 700 は残す）。
/// [error] は group の下に error_text で出し、出たときに最初の chip へフォーカスする。
/// 選択肢を読んでいるあいだ・読めないときは chip を出さず、呼ぶ側が loading の section_text・select の load_failed の文言を出す。
class RallyChoiceChips<T> extends StatefulWidget {
  const RallyChoiceChips({
    super.key,
    required this.label,
    required this.options,
    required this.selected,
    required this.onSelected,
    this.disabled = false,
    this.error,
  });

  /// group のラベル（text_field の label と同じ small 700 text）
  final String label;
  final List<RallyChipOption<T>> options;
  final T? selected;
  final ValueChanged<T> onSelected;
  final bool disabled;
  final String? error;

  /// 選ばずに送ったときの文言（chip.states.error）
  static String notSelectedMessage(String label) =>
      '$labelが選ばれていません。1 つ選んでください。';

  @override
  State<RallyChoiceChips<T>> createState() => _RallyChoiceChipsState<T>();
}

class _RallyChoiceChipsState<T> extends State<RallyChoiceChips<T>> {
  final _firstFocus = FocusNode();

  @override
  void didUpdateWidget(RallyChoiceChips<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.error == null && widget.error != null) {
      _firstFocus.requestFocus();
    }
  }

  @override
  void dispose() {
    _firstFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final error = widget.error;
    return Semantics(
      container: true,
      label: widget.label,
      explicitChildNodes: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          ExcludeSemantics(
            child: Text(
              widget.label,
              style: RallyTypography.small.copyWith(
                fontWeight: FontWeight.w700,
                color: RallyColors.text,
              ),
            ),
          ),
          const SizedBox(height: RallySpacing.xs),
          Wrap(
            spacing: RallySpacing.sm,
            runSpacing: RallySpacing.sm,
            children: [
              for (final (index, option) in widget.options.indexed)
                _Chip(
                  label: option.label,
                  on: option.value == widget.selected,
                  focusNode: index == 0 ? _firstFocus : null,
                  onPressed: widget.disabled
                      ? null
                      : () => widget.onSelected(option.value),
                ),
            ],
          ),
          if (error != null)
            Padding(
              padding: const EdgeInsets.only(top: RallySpacing.xs),
              child: Semantics(
                liveRegion: true,
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  spacing: RallySpacing.xs,
                  children: [
                    Icon(
                      Icons.error_outline_rounded,
                      size: 20,
                      color: RallyColors.danger.fg,
                    ),
                    Expanded(
                      child: Text(
                        error,
                        style: RallyTypography.small.copyWith(
                          color: RallyColors.danger.fg,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// chip 1 つ。高さ 48px・左右 md 16px・角丸 pill・border_strong 1px・small。
/// 押下は off が surface、on が primary_container_pressed。フォーカスは focus_ring 2px。
class _Chip extends StatefulWidget {
  const _Chip({
    required this.label,
    required this.on,
    required this.onPressed,
    this.focusNode,
  });

  final String label;
  final bool on;
  final VoidCallback? onPressed;
  final FocusNode? focusNode;

  @override
  State<_Chip> createState() => _ChipState();
}

class _ChipState extends State<_Chip> {
  var _pressed = false;
  var _focused = false;

  @override
  Widget build(BuildContext context) {
    final disabled = widget.onPressed == null;
    final on = widget.on;
    final Color container;
    if (disabled) {
      container = RallyColors.surface;
    } else if (on) {
      container = _pressed
          ? RallyColors.primaryContainerPressed
          : RallyColors.primaryContainer;
    } else {
      container = _pressed ? RallyColors.surface : RallyColors.background;
    }
    final foreground = disabled
        ? RallyColors.textMuted
        : on
        ? RallyColors.onPrimaryContainer
        : RallyColors.text;
    final BorderSide side;
    if (_focused) {
      side = const BorderSide(color: RallyColors.focusRing, width: 2);
    } else if (disabled) {
      side = BorderSide.none;
    } else {
      side = const BorderSide(color: RallyColors.borderStrong);
    }
    return Semantics(
      inMutuallyExclusiveGroup: true,
      checked: on,
      button: true,
      enabled: !disabled,
      focusable: !disabled,
      focused: _focused,
      onTap: widget.onPressed,
      excludeSemantics: true,
      label: on ? '${widget.label}、選択中' : widget.label,
      child: Material(
        color: container,
        animationDuration: RallyMotion.fast,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(RallyRadius.pill),
          side: side,
        ),
        child: InkWell(
          focusNode: widget.focusNode,
          onTap: widget.onPressed,
          onHighlightChanged: (value) => setState(() => _pressed = value),
          onFocusChange: (value) => setState(() => _focused = value),
          customBorder: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(RallyRadius.pill),
          ),
          splashFactory: NoSplash.splashFactory,
          overlayColor: const WidgetStatePropertyAll(Colors.transparent),
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              minHeight: RallySpacing.touchTarget,
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: RallySpacing.md),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                spacing: RallySpacing.xs,
                children: [
                  if (on)
                    Icon(Icons.check_rounded, size: 20, color: foreground),
                  Text(
                    widget.label,
                    style: RallyTypography.small.copyWith(
                      color: foreground,
                      fontWeight: on ? FontWeight.w700 : FontWeight.w400,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
