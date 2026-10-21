import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/music_assistant.dart';
import 'package:pinepods_mobile/services/music_assistant/music_assistant_api.dart';

/// Music Assistant connection form. Always shown: this is how the feature is
/// turned on. Speaker controls stay hidden until the bridge is enabled.
class MusicAssistantSettings extends StatefulWidget {
  const MusicAssistantSettings({super.key, this.api, this.userId});

  /// Overridden in tests. Production reads the signed-in session.
  final MusicAssistantApi? api;
  final int? userId;

  @override
  State<MusicAssistantSettings> createState() => _MusicAssistantSettingsState();
}

class _MusicAssistantSettingsState extends State<MusicAssistantSettings> {
  final _server = TextEditingController();
  final _external = TextEditingController();
  final _token = TextEditingController();
  bool _enabled = false;
  bool _hasToken = false;
  bool _clearToken = false;
  bool _connected = false;
  bool _loading = true;
  bool _saving = false;
  bool _testing = false;
  String? _version;
  String? _warning;
  String? _lastError;
  TestExplanation? _result;
  String? _loadError;

  MusicAssistantApi? _api;
  int? _userId;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _api ??= widget.api;
    _userId ??= widget.userId;
    if (_api == null || _userId == null) {
      final session = maSessionOf(context);
      if (session != null) {
        _api ??= MusicAssistantApi(server: session.server, apiKey: session.apiKey);
        _userId ??= session.userId;
      }
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  @override
  void dispose() {
    _server.dispose();
    _external.dispose();
    _token.dispose();
    super.dispose();
  }

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
      MusicAssistantController.instance.applySettings(view);
      setState(() {
        _enabled = view.enabled;
        _server.text = view.serverUrl ?? '';
        _external.text = view.externalBaseUrl ?? '';
        _hasToken = view.hasToken;
        _connected = view.connected;
        _version = view.serverVersion;
        _warning = view.warning;
        _lastError = view.lastError;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _loadError = err.toString();
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
        serverUrl: _server.text,
        externalBaseUrl: _external.text,
        token: _token.text,
        clearToken: _clearToken,
      );
      MusicAssistantController.instance.applySettings(view);
      if (view.enabled) {
        MusicAssistantController.instance.applyPlayers(await api.listPlayers(userId));
      }
      if (!mounted) return;
      setState(() {
        _hasToken = view.hasToken;
        _connected = view.connected;
        _version = view.serverVersion;
        _warning = view.warning;
        _lastError = view.lastError;
        _token.clear();
        _clearToken = false;
        _result = null;
        _saving = false;
      });
      ScaffoldMessenger.maybeOf(context)?.showSnackBar(
        const SnackBar(content: Text('Music Assistant settings saved.')),
      );
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _result = explainTest(externalBase: _external.text, errorText: err.toString());
      });
    }
  }

  Future<void> _test() async {
    final api = _api;
    final userId = _userId;
    if (api == null || userId == null) return;
    setState(() => _testing = true);
    // A rejected external URL is explained locally and left off the probe, so
    // the probe can still say whether Music Assistant itself is up.
    final probeExternal = externalBaseIsRejected(_external.text) ? '' : _external.text;
    try {
      final report = await api.testConnection(
        userId: userId,
        serverUrl: _server.text,
        externalBaseUrl: probeExternal,
        token: _token.text,
      );
      if (!mounted) return;
      setState(() {
        _testing = false;
        _result = explainTest(externalBase: _external.text, probe: report);
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _testing = false;
        _result = explainTest(externalBase: _external.text, errorText: err.toString());
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_api == null || _userId == null) {
      return const ListTile(
        leading: Icon(Icons.speaker),
        title: Text('Music Assistant'),
        subtitle: Text('Sign in to PinePods to connect speakers.'),
      );
    }
    final result = _result;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SwitchListTile.adaptive(
            contentPadding: EdgeInsets.zero,
            secondary: const Icon(Icons.speaker),
            title: const Text('Enable Music Assistant'),
            subtitle: const Text(
              'Play episodes on Music Assistant speakers and groups. Off until you save an enabled connection.',
            ),
            value: _enabled,
            onChanged: _loading ? null : (value) => setState(() => _enabled = value),
          ),
          TextField(
            controller: _server,
            decoration: const InputDecoration(
              labelText: 'Server URL',
              hintText: 'http://192.168.1.20:8095',
              helperText: 'Music Assistant origin. A missing port becomes 8095.',
            ),
            keyboardType: TextInputType.url,
            autocorrect: false,
          ),
          TextField(
            controller: _token,
            obscureText: true,
            decoration: InputDecoration(
              labelText: 'Access token',
              hintText: _hasToken ? 'Saved — leave blank to keep' : null,
              helperText: 'Long-lived token from Music Assistant → Settings → Profile.',
            ),
            autocorrect: false,
          ),
          if (_hasToken)
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Remove saved token'),
              value: _clearToken,
              onChanged: (value) => setState(() => _clearToken = value ?? false),
            ),
          TextField(
            controller: _external,
            decoration: const InputDecoration(
              labelText: 'PinePods URL for Music Assistant',
              hintText: 'http://192.168.1.10:8040',
              helperText:
                  'Origin Music Assistant uses to fetch episodes. Use a LAN or public address, not localhost.',
            ),
            keyboardType: TextInputType.url,
            autocorrect: false,
          ),
          const Padding(
            padding: EdgeInsets.only(top: 8),
            child: Text(
              'Music Assistant has to open this URL itself. If playback starts and stops immediately, the Music Assistant host cannot reach it.',
              style: TextStyle(fontSize: 12),
            ),
          ),
          if (_loading)
            const Padding(
              padding: EdgeInsets.only(top: 12),
              child: LinearProgressIndicator(),
            ),
          if (_loadError != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _loadError!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          if (!_loading)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(_connected ? 'Connected' : 'Not connected yet'),
            ),
          if (_version != null) Text('Server $_version'),
          if (_warning != null)
            Text('Warning: $_warning', style: const TextStyle(fontSize: 12)),
          if (_lastError != null)
            Text(
              _lastError!,
              style: TextStyle(color: Theme.of(context).colorScheme.error, fontSize: 12),
            ),
          if (result != null) _TestResultView(result: result),
          const SizedBox(height: 8),
          Row(
            children: [
              OutlinedButton(
                onPressed: _testing ? null : _test,
                child: Text(_testing ? 'Testing…' : 'Test connection'),
              ),
              const SizedBox(width: 8),
              FilledButton(
                onPressed: _saving ? null : _save,
                child: Text(_saving ? 'Saving…' : 'Save'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _TestResultView extends StatelessWidget {
  const _TestResultView({required this.result});

  final TestExplanation result;

  @override
  Widget build(BuildContext context) {
    final error = Theme.of(context).colorScheme.error;
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (result.ok)
            Text('Connected to Music Assistant.', style: TextStyle(color: Theme.of(context).colorScheme.primary)),
          for (final problem in result.problems)
            if (problem != MaProblem.streamHint)
              Text(_problemText(problem), style: TextStyle(color: error)),
          if (result.detail.isNotEmpty)
            Text(result.detail, style: const TextStyle(fontSize: 12)),
          if (result.showStreamHint)
            const Text(
              'Music Assistant has to open the PinePods URL itself. If playback starts and stops immediately, that host cannot reach it.',
              style: TextStyle(fontSize: 12),
            ),
        ],
      ),
    );
  }
}

String _problemText(MaProblem problem) {
  switch (problem) {
    case MaProblem.localhostRejected:
      return 'That address was rejected. Music Assistant cannot use localhost or a link-local address to fetch PinePods.';
    case MaProblem.unreachable:
      return 'Music Assistant is unreachable. Check the server URL and that PinePods can open its WebSocket.';
    case MaProblem.streamHint:
      return 'Music Assistant cannot fetch the stream URL.';
    case MaProblem.notConfigured:
      return 'Add a server URL, a token, and the PinePods URL before enabling.';
    case MaProblem.authFailed:
      return 'Music Assistant rejected the token.';
    case MaProblem.other:
      return 'Music Assistant command failed.';
  }
}
