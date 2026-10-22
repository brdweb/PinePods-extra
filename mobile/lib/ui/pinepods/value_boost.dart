import 'package:flutter/material.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/value4value/value4value_api.dart';

/// One-time boost. Hidden when the episode has no payable value block.
/// The send button stays disabled until the listener confirms the sats are real.
class ValueBoost extends StatefulWidget {
  const ValueBoost({super.key, required this.episodeId});

  final int episodeId;

  @override
  State<ValueBoost> createState() => _ValueBoostState();
}

class _ValueBoostState extends State<ValueBoost> {
  final _sats = TextEditingController();
  final _message = TextEditingController();
  bool _loading = true;
  bool _payable = false;
  bool _confirmed = false;
  bool _sending = false;
  int? _suggested;
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
    _sats.dispose();
    _message.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    final api = _api;
    final userId = _userId;
    if (api == null || userId == null || widget.episodeId <= 0) {
      if (mounted) setState(() => _loading = false);
      return;
    }
    try {
      final preview = await api.previewEpisode(userId: userId, episodeId: widget.episodeId);
      if (!mounted) return;
      setState(() {
        _payable = preview.payable;
        _suggested = preview.suggestedSats;
        if (preview.suggestedSats != null && _sats.text.isEmpty) {
          _sats.text = '${preview.suggestedSats}';
        }
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _send() async {
    final api = _api;
    final userId = _userId;
    final amount = int.tryParse(_sats.text.trim()) ?? 0;
    if (api == null || userId == null || amount <= 0 || !_confirmed) return;
    setState(() => _sending = true);
    try {
      final report = await api.boost(
        userId: userId,
        episodeId: widget.episodeId,
        sats: amount,
        message: _message.text,
        senderName: '',
        positionSeconds: 0,
      );
      if (!mounted) return;
      setState(() {
        _sending = false;
        _confirmed = false;
        _noticeOk = report.sent;
        _notice = report.sent
            ? L.of(context)!.v4v_sent('${report.satsSent}')
            : (report.reason.isEmpty ? L.of(context)!.v4v_failed : report.reason);
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _sending = false;
        _noticeOk = false;
        _notice = '$err';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading || !_payable) return const SizedBox.shrink();
    final l10n = L.of(context)!;
    final amount = int.tryParse(_sats.text.trim()) ?? 0;
    return Card(
      margin: const EdgeInsets.only(top: 16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(l10n.v4v_boost, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(l10n.v4v_real),
            if (_suggested != null) Text(l10n.v4v_suggested('$_suggested')),
            TextField(
              controller: _sats,
              keyboardType: TextInputType.number,
              decoration: InputDecoration(labelText: l10n.v4v_amount),
              onChanged: (_) => setState(() {}),
            ),
            TextField(
              controller: _message,
              decoration: InputDecoration(labelText: l10n.v4v_message),
            ),
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l10n.v4v_confirm),
              value: _confirmed,
              onChanged: (value) => setState(() => _confirmed = value ?? false),
            ),
            FilledButton(
              onPressed: _sending || !_confirmed || amount <= 0 ? null : _send,
              child: Text(l10n.v4v_send),
            ),
            if (_notice != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  _notice!,
                  style: TextStyle(color: _noticeOk ? Colors.green : Theme.of(context).colorScheme.error),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
