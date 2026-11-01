import 'package:flutter/material.dart';

/// 1 行・複数行の入力（components.yaml の input の single_line / multi_line）。
/// 見出し [label] を必ず付ける。[errorText] があれば入力の誤り（invalid）として項目の下に出し、[enabled] が false なら無効。
class RallyTextInput extends StatelessWidget {
  const RallyTextInput({
    super.key,
    required this.label,
    this.controller,
    this.initialValue,
    this.onChanged,
    this.errorText,
    this.enabled = true,
    this.multiLine = false,
  }) : assert(controller == null || initialValue == null);

  final String label;
  final TextEditingController? controller;
  final String? initialValue;
  final ValueChanged<String>? onChanged;
  final String? errorText;
  final bool enabled;

  /// 複数行（multi_line）。3 行の高さから、入れた分だけ伸びる
  final bool multiLine;

  @override
  Widget build(BuildContext context) => TextFormField(
    controller: controller,
    initialValue: initialValue,
    onChanged: onChanged,
    enabled: enabled,
    minLines: multiLine ? 3 : 1,
    maxLines: multiLine ? null : 1,
    keyboardType: multiLine ? TextInputType.multiline : TextInputType.text,
    decoration: InputDecoration(labelText: label, errorText: errorText),
  );
}

/// 選択（components.yaml の input の select）。決まった選択肢 [options]（値と表示の組）から 1 つ選ぶ。
class RallySelect<T> extends StatelessWidget {
  const RallySelect({
    super.key,
    required this.label,
    required this.options,
    this.value,
    this.onChanged,
    this.errorText,
    this.enabled = true,
  });

  final String label;
  final List<(T, String)> options;
  final T? value;
  final ValueChanged<T?>? onChanged;
  final String? errorText;
  final bool enabled;

  @override
  Widget build(BuildContext context) => DropdownMenu<T>(
    label: Text(label),
    initialSelection: value,
    enabled: enabled,
    errorText: errorText,
    onSelected: onChanged,
    dropdownMenuEntries: [
      for (final (optionValue, optionLabel) in options)
        DropdownMenuEntry<T>(value: optionValue, label: optionLabel),
    ],
  );
}
