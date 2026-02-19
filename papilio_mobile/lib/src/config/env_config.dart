import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:shared_preferences/shared_preferences.dart';

part 'env_config.g.dart';

class EnvConfig {
  final String? serverUrl;
  final bool isDataSaverMode;
  
  EnvConfig({
    this.serverUrl, 
    this.isDataSaverMode = false,
  });

  String get _cleanServerUrl {
    if (serverUrl == null) return '';
    String url = serverUrl!.trim();
    return url.endsWith('/') ? url.substring(0, url.length - 1) : url;
  }

  bool get isConfigured => serverUrl != null && serverUrl!.isNotEmpty;
  
  String get apiBaseUrl => "$_cleanServerUrl/api/";
  String get coversBaseUrl => "$_cleanServerUrl/api/music/covers/";
  String get avatarsBaseUrl => "$_cleanServerUrl/data/avatars/";
  String get musicBaseUrl => "$_cleanServerUrl/data/music/";
  String get streamBaseUrl => "$_cleanServerUrl/api/music/stream/";

  String? getEffectiveImageUrl(String? imageUrl) {
    if (imageUrl == null) return null;
    if (imageUrl.startsWith('http')) return imageUrl;
    
    final cleanPath = imageUrl.startsWith('/') ? imageUrl.substring(1) : imageUrl;
    // 如果路径包含斜杠，说明是在曲库子目录下
    if (cleanPath.contains('/')) {
      return "$musicBaseUrl$cleanPath";
    } else {
      return "$avatarsBaseUrl$cleanPath";
    }
  }
}

@riverpod
class EnvConfigNotifier extends _$EnvConfigNotifier {
  @override
  EnvConfig build() {
    _load();
    return EnvConfig();
  }

  Future<void> _load() async {
    final prefs = await SharedPreferences.getInstance();
    final url = prefs.getString('server_url');
    final saver = prefs.getBool('is_data_saver') ?? false;
    state = EnvConfig(serverUrl: url, isDataSaverMode: saver);
  }

  Future<void> updateServerUrl(String newUrl) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('server_url', newUrl);
    state = EnvConfig(serverUrl: newUrl, isDataSaverMode: state.isDataSaverMode);
  }

  Future<void> toggleDataSaver(bool enabled) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('is_data_saver', enabled);
    state = EnvConfig(serverUrl: state.serverUrl, isDataSaverMode: enabled);
  }

  Future<void> resetConfig() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove('server_url');
    // Force set state to unconfigured to trigger AuthGate rebuild
    state = EnvConfig(serverUrl: null, isDataSaverMode: state.isDataSaverMode);
    debugPrint('Server configuration has been reset');
  }
}
