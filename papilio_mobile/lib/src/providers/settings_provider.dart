import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

final sharedPreferencesProvider = Provider<SharedPreferences>((ref) => throw UnimplementedError());

class SettingsNotifier extends StateNotifier<SettingsState> {
  final SharedPreferences _prefs;

  SettingsNotifier(this._prefs) : super(SettingsState.load(_prefs));

  Future<void> setGlobalLyricOffset(int offsetMs) async {
    await _prefs.setInt('global_lyric_offset_ms', offsetMs);
    state = state.copyWith(globalLyricOffsetMs: offsetMs);
  }
}

class SettingsState {
  final int globalLyricOffsetMs;

  SettingsState({required this.globalLyricOffsetMs});

  factory SettingsState.load(SharedPreferences prefs) {
    return SettingsState(
      globalLyricOffsetMs: prefs.getInt('global_lyric_offset_ms') ?? 0,
    );
  }

  SettingsState copyWith({int? globalLyricOffsetMs}) {
    return SettingsState(
      globalLyricOffsetMs: globalLyricOffsetMs ?? this.globalLyricOffsetMs,
    );
  }
}

final settingsProvider = StateNotifierProvider<SettingsNotifier, SettingsState>((ref) {
  final prefs = ref.watch(sharedPreferencesProvider);
  return SettingsNotifier(prefs);
});
