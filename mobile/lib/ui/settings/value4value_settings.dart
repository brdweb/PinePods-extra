import 'package:flutter/material.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/value4value/value4value_api.dart';

/// Wallet connection for Value for Value. Nothing is sent until this is on
/// and a daily limit above zero is saved.
class Value4ValueSettings extends StatefulWidget {
  const Value4ValueSettings({super.key});

  @override
  State<Value4ValueSettings> createState() => _Value4ValueSettingsState();
}

class _Value4ValueSettingsState extends State<Value4ValueSettings> {
  final _secret = TextEditingController();
  final _maxBoost = TextEditingController(text: '1000');
  final _stream = TextEditingController(text: '0');
  final _daily = TextEditingController(text: '0');
  bool _enabled = false;
  bool _hasSecret = false;
  bool _clearSecret = false;
  bool _loading = true;
  bool _saving = false;
  int _spent = 0;
  String? _notice;
  bool _noticeOk = false;
  ValueApi? _api;
  int? _userId;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final session = valueSessionOf(context);
    if (session != null) {
      _api ??= ValueApi(server: session.server, apiKey: session.apiKey);
      _userId ??= session.userId;
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  @override
  void dispose() {
    _secret.dispose();
    _maxBoost.dispose();
    _stream.dispose();
    _daily.dispose();
    super.dispose();
  }

  int _parse(TextEditingController controller) => int.tryParse(controller.text.trim()) ?? 0;

  Future<void> _load() async {
    final api = _api;
    final userId = _userId;
    if (api == null || userId == null) {
      if (mounted) setState(() => _loading = false);
      return;
    }
    try {
      final view = await api.getSettings(userId);
      if (!mounted) return;
      setState(() {
        _enabled = view.enabled;
        _hasSecret = view.hasSecret;
        _maxBoost.text = '${view.maxBoostSats}';
        _stream.text = '${view.streamSatsPerMinute}';
        _daily.text = '${view.dailyLimitSats}';
        _spent = view.spentTodaySats;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _noticeOk = false;
        _notice = '$err';
      });
    }
  }

  Future<void> _save() async {
    final api = _api;
    final userId = _userId;
    if (api == null || userId == null) return;
    setState(() => _saving = true);
    try {
      final view = await api.saveSettings(
        userId: userId,
        enabled: _enabled,
        secret: _secret.text,
        clearSecret: _clearSecret,
        maxBoostSats: _parse(_maxBoost),
        streamSatsPerMinute: _parse(_stream),
        dailyLimitSats: _parse(_daily),
      );
      if (!mounted) return;
      setState(() {
        _hasSecret = view.hasSecret;
        _spent = view.spentTodaySats;
        _secret.clear();
        _clearSecret = false;
        _noticeOk = true;
        _notice = L.of(context)!.v4v_saved;
        _saving = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _noticeOk = false;
        _notice = '$err';
      });
    }
  }

  Future<void> _test() async {
    final api = _api;
    final userId = _userId;
    if (api == null || userId == null) return;
    setState(() => _saving = true);
    try {
      final report = await api.testWallet(userId: userId, secret: _secret.text);
      if (!mounted) return;
      setState(() {
        _saving = false;
        _noticeOk = report.ok;
        _notice = report.ok
            ? L.of(context)!.v4v_balance('${report.balanceSats}')
            : report.detail;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _noticeOk = false;
        _notice = '$err';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = L.of(context)!;
    if (_loading) {
      return const Padding(
        padding: EdgeInsets.all(16),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.v4v_banner),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l10n.v4v_enabled),
            value: _enabled,
            onChanged: (value) => setState(() => _enabled = value),
          ),
          TextField(
            controller: _secret,
            obscureText: true,
            decoration: InputDecoration(
              labelText: l10n.v4v_secret,
              hintText: _hasSecret ? l10n.v4v_secret_stored : null,
            ),
          ),
          if (_hasSecret)
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l10n.v4v_clear_secret),
              value: _clearSecret,
              onChanged: (value) => setState(() => _clearSecret = value ?? false),
            ),
          TextField(
            controller: _maxBoost,
            keyboardType: TextInputType.number,
            decoration: InputDecoration(labelText: l10n.v4v_max_boost),
          ),
          TextField(
            controller: _stream,
            keyboardType: TextInputType.number,
            decoration: InputDecoration(labelText: l10n.v4v_stream),
          ),
          TextField(
            controller: _daily,
            keyboardType: TextInputType.number,
            decoration: InputDecoration(labelText: l10n.v4v_daily),
          ),
          const SizedBox(height: 8),
          Text(l10n.v4v_spent('$_spent')),
          if (_notice != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _notice!,
                style: TextStyle(color: _noticeOk ? Colors.green : Theme.of(context).colorScheme.error),
              ),
            ),
          const SizedBox(height: 8),
          Row(
            children: [
              OutlinedButton(onPressed: _saving ? null : _test, child: Text(l10n.v4v_test)),
              const SizedBox(width: 8),
              FilledButton(onPressed: _saving ? null : _save, child: Text(l10n.v4v_save)),
            ],
          ),
        ],
      ),
    );
  }
}
