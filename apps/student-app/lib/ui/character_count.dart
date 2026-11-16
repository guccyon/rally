import 'package:flutter/material.dart';

import 'labels.dart';

/// 文字数の上限がある入力欄の [InputDecoration] に、文字数の表示（`0/2000`）と読み上げ（`残り 2000 文字`）を入れる。
/// Flutter の既定の読み上げは英語（`2000 characters remaining`）なので、読み上げだけを日本語にする。
/// 表示と数え方（書記素で数える）は Flutter の既定と同じ。入力欄の `maxLength` には同じ [maxLength] を渡し、
/// [text] が変わるたびに作り直す（`counterText` を渡すと、入力欄は自前の数を出さない）
InputDecoration withCharacterCount(
  InputDecoration decoration, {
  required String text,
  required int maxLength,
}) {
  final length = text.characters.length;
  final remaining = (maxLength - length).clamp(0, maxLength);
  return decoration.copyWith(
    counterText: '$length/$maxLength',
    semanticCounterText: remainingCharactersLabel(remaining),
  );
}
