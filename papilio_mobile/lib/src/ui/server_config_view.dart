import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:dio/dio.dart';
import '../config/env_config.dart';
import 'widgets/gradient_scaffold.dart';

class ServerConfigView extends ConsumerStatefulWidget {
  const ServerConfigView({super.key});

  @override
  ConsumerState<ServerConfigView> createState() => _ServerConfigViewState();
}

class _ServerConfigViewState extends ConsumerState<ServerConfigView> {
  final _urlController = TextEditingController();
  bool _isConnecting = false;

  @override
  void initState() {
    super.initState();
    _urlController.text = ref.read(envConfigNotifierProvider).serverUrl ?? 'http://';
  }

  Future<void> _save() async {
    final url = _urlController.text.trim();
    if (url.isEmpty || !url.startsWith('http')) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('请输入有效的服务器地址 (以 http:// 或 https:// 开头)'))
      );
      return;
    }
    
    // Remove trailing slash if present for testing
    final normalizedUrl = url.endsWith('/') ? url.substring(0, url.length - 1) : url;
    
    setState(() => _isConnecting = true);
    
    try {
      // 验证地址连通性
      final dio = Dio(BaseOptions(connectTimeout: const Duration(seconds: 5)));
      final testUrl = '$normalizedUrl/api/health';
      debugPrint('Testing connectivity to: $testUrl');
      
      final response = await dio.get(testUrl);
      
      if (response.statusCode == 200) {
        ref.read(envConfigNotifierProvider.notifier).updateServerUrl(normalizedUrl);
      } else {
        throw Exception('HTTP ${response.statusCode}');
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('无法连接到服务器: $e'),
            backgroundColor: Colors.redAccent,
          )
        );
      }
    } finally {
      if (mounted) setState(() => _isConnecting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return GradientScaffold(
      body: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 32),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.lan_rounded, size: 64, color: Theme.of(context).colorScheme.primary),
            const SizedBox(height: 24),
            const Text('连接到 Papilio', style: TextStyle(fontSize: 28, fontWeight: FontWeight.bold)),
            const SizedBox(height: 8),
            const Text('请输入您的 Papilio 服务器地址以开始使用。', style: TextStyle(color: Colors.white54)),
            const SizedBox(height: 32),
            TextField(
              controller: _urlController,
              enabled: !_isConnecting,
              decoration: InputDecoration(
                labelText: '服务器地址',
                hintText: 'http://your-server-ip:3000',
                filled: true,
                fillColor: Colors.white.withOpacity(0.05),
                border: OutlineInputBorder(borderRadius: BorderRadius.circular(16), borderSide: BorderSide.none),
              ),
              keyboardType: TextInputType.url,
            ),
            const SizedBox(height: 32),
            SizedBox(
              width: double.infinity,
              height: 56,
              child: ElevatedButton(
                onPressed: _isConnecting ? null : _save,
                style: ElevatedButton.styleFrom(
                  backgroundColor: Theme.of(context).colorScheme.primary,
                  foregroundColor: Colors.white,
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
                ),
                child: _isConnecting 
                  ? const SizedBox(width: 24, height: 24, child: CircularProgressIndicator(color: Colors.white, strokeWidth: 2))
                  : const Text('连接', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
              ),
            ),
          ],
        ),
      ),
    );
  }
}