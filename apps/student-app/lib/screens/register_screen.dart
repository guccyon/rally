import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/character_count.dart';
import '../ui/flow_scaffold.dart';
import '../ui/sport_selector.dart';

/// アカウント登録（`registerAccount`）
class RegisterScreen extends ConsumerStatefulWidget {
  const RegisterScreen({super.key});

  @override
  ConsumerState<RegisterScreen> createState() => _RegisterScreenState();
}

class _RegisterScreenState extends ConsumerState<RegisterScreen> {
  /// 表示名の入力欄の文字数の上限
  static const _displayNameMaxLength = 50;

  final _formKey = GlobalKey<FormState>();
  final _email = TextEditingController();
  final _displayName = TextEditingController();
  Sport? _sport;
  bool _sportMissing = false;

  @override
  void dispose() {
    _email.dispose();
    _displayName.dispose();
    super.dispose();
  }

  void _submit() {
    final valid = _formKey.currentState!.validate();
    setState(() => _sportMissing = _sport == null);
    if (!valid || _sport == null) return;
    ref
        .read(studentFlowProvider.notifier)
        .register(
          email: _email.text.trim(),
          displayName: _displayName.text.trim(),
          sport: _sport!,
        );
  }

  @override
  Widget build(BuildContext context) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    return FlowScaffold(
      title: 'アカウント登録',
      children: [
        Form(
          key: _formKey,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TextFormField(
                controller: _email,
                decoration: const InputDecoration(labelText: 'メールアドレス'),
                keyboardType: TextInputType.emailAddress,
                validator: (value) =>
                    (value ?? '').contains('@') ? null : 'メールアドレスを入力してください',
              ),
              // 文字数の残りの読み上げを入力に合わせて作り直す
              ValueListenableBuilder<TextEditingValue>(
                valueListenable: _displayName,
                builder: (context, value, _) => TextFormField(
                  controller: _displayName,
                  decoration: withCharacterCount(
                    const InputDecoration(labelText: '表示名'),
                    text: value.text,
                    maxLength: _displayNameMaxLength,
                  ),
                  maxLength: _displayNameMaxLength,
                  validator: (value) =>
                      (value ?? '').trim().isEmpty ? '表示名を入力してください' : null,
                ),
              ),
              const SizedBox(height: 8),
              const Text('習いたいスポーツ'),
              SportSelector(
                selected: _sport,
                onSelected: (sport) => setState(() {
                  _sport = sport;
                  _sportMissing = false;
                }),
              ),
              if (_sportMissing)
                Text(
                  'スポーツを選んでください',
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              const SizedBox(height: 16),
              FilledButton(
                onPressed: busy ? null : _submit,
                child: const Text('登録する'),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
