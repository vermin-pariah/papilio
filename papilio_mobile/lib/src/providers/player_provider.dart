import 'package:audio_service/audio_service.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'dart:async';
import '../api/audio_handler.dart';
import '../models/track.dart';
import '../config/env_config.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../api/music_repository.dart';

part 'player_provider.g.dart';

AudioPlayerHandler? _globalAudioHandler;
void setGlobalAudioHandler(AudioPlayerHandler h) => _globalAudioHandler = h;

@Riverpod(keepAlive: true)
AudioPlayerHandler? playerHandler(PlayerHandlerRef ref) => _globalAudioHandler;

@riverpod
Stream<MediaItem?> currentTrack(CurrentTrackRef ref) {
  final handler = ref.watch(playerHandlerProvider);
  return handler?.mediaItem.stream ?? const Stream.empty();
}

@riverpod
Stream<PlaybackState?> playbackState(PlaybackStateRef ref) {
  final handler = ref.watch(playerHandlerProvider);
  return handler?.playbackState.stream ?? const Stream.empty();
}

@riverpod
Stream<List<MediaItem>> queue(QueueRef ref) {
  final handler = ref.watch(playerHandlerProvider);
  return handler?.queue.asBroadcastStream() ?? const Stream.empty();
}

// 核心改进：专门用于进度条轮询的 Provider，避免匿名 Provider 导致内存泄漏
final playbackProgressTickerProvider = StreamProvider<int>((ref) {
  return Stream.periodic(const Duration(milliseconds: 200), (i) => i);
});

@riverpod
PlayerController? playerController(PlayerControllerRef ref) {
  final h = ref.watch(playerHandlerProvider);
  if (h == null) return null;
  final c = ref.watch(envConfigNotifierProvider);
  return PlayerController(h, c, ref);
}

class PlayerController {
  final AudioPlayerHandler _handler;
  final EnvConfig _config;
  final PlayerControllerRef _ref;
  Timer? _cloudSyncTimer;
  bool _isInitializing = false;
  bool _hasInteractedInSession = false;
  int _currentActionId = 0;

  PlayerController(this._handler, this._config, this._ref);

  bool _shouldAbortInitialization() {
    return _hasInteractedInSession || _handler.playbackState.value.playing || _handler.queue.value.isNotEmpty;
  }

  Future<void> restoreLastState() async {
    if (_isInitializing || _shouldAbortInitialization()) return;
    _isInitializing = true;
    
    try {
      final repo = _ref.read(musicRepositoryProvider);
      final cloud = await repo.getCloudPlayback();
      
      if (_shouldAbortInitialization()) return;

      String? trackId = cloud?['track_id'];
      int posMs = cloud?['position_ms'] ?? 0;

      if (trackId == null) {
        final prefs = await SharedPreferences.getInstance();
        trackId = prefs.getString('last_track_id');
        posMs = prefs.getInt('last_position_ms') ?? 0;
      }

      if (_shouldAbortInitialization()) return;

      if (trackId != null) {
        final tracks = await repo.getTracks(query: trackId);
        if (tracks.isNotEmpty && !_shouldAbortInitialization()) {
          await _handler.setQueue([tracks.first], 0, _config);
          await _handler.seek(Duration(milliseconds: posMs));
          _handler.pause();
          return;
        }
      }
      
      if (!_shouldAbortInitialization()) {
        await playRandomly(isAuto: true);
      }
    } catch (_) {
      if (!_shouldAbortInitialization()) {
        await playRandomly(isAuto: true);
      }
    } finally {
      _isInitializing = false;
    }
  }

  Future<void> playRandomly({bool isAuto = false}) async {
    if (isAuto && _shouldAbortInitialization()) return;
    
    try {
      final repo = _ref.read(musicRepositoryProvider);
      final tracks = await repo.getTracks(limit: 50); 
      if (tracks.isNotEmpty && (!isAuto || !_shouldAbortInitialization())) {
        tracks.shuffle();
        await _handler.setQueue(tracks, 0, _config);
        startCloudSync();
      }
    } catch (_) {}
  }

  void startCloudSync() {
    _cloudSyncTimer?.cancel();
    _cloudSyncTimer = Timer.periodic(const Duration(seconds: 15), (_) => _syncToCloud());
  }

  void stopCloudSync() {
    _cloudSyncTimer?.cancel();
    _syncToCloud();
  }

  Future<void> _syncToCloud() async {
    final item = _handler.mediaItem.value;
    final state = _handler.playbackState.value;
    if (item != null) {
      _ref.read(musicRepositoryProvider).updateCloudPlayback(item.id, state.position.inMilliseconds);
    }
  }

  Future<void> playTrack(Track t) async { 
    _hasInteractedInSession = true;
    _currentActionId++;
    final actionId = _currentActionId;
    await _handler.playTrack(t, _config); 
    if (actionId == _currentActionId) {
      startCloudSync(); 
      _ref.read(musicRepositoryProvider).recordPlay(t.id);
    }
  }

  Future<void> playQueue(List<Track> l, int i) async { 
    _hasInteractedInSession = true;
    _currentActionId++;
    final actionId = _currentActionId;
    await _handler.setQueue(l, i, _config); 
    if (actionId == _currentActionId) {
      startCloudSync(); 
      if (l.isNotEmpty && i < l.length) {
        _ref.read(musicRepositoryProvider).recordPlay(l[i].id);
      }
    }
  }

  void togglePlay() {
    if (_handler.playbackState.value.playing) { _handler.pause(); stopCloudSync(); }
    else { _handler.play(); startCloudSync(); }
  }

  void pause() {
    _handler.pause();
    stopCloudSync();
  }

  void skipToNext() => _handler.skipToNext();
  void skipToPrevious() => _handler.skipToPrevious();
  void skipToQueueItem(int i) => _handler.skipToQueueItem(i);
  void seek(Duration p) => _handler.seek(p);

  void toggleRepeatMode() {
    final curr = _handler.playbackState.value.repeatMode;
    final next = curr == AudioServiceRepeatMode.none ? AudioServiceRepeatMode.all : (curr == AudioServiceRepeatMode.all ? AudioServiceRepeatMode.one : AudioServiceRepeatMode.none);
    _handler.setRepeatMode(next);
  }

  void toggleShuffleMode() {
    final next = _handler.playbackState.value.shuffleMode == AudioServiceShuffleMode.none ? AudioServiceShuffleMode.all : AudioServiceShuffleMode.none;
    _handler.setShuffleMode(next);
  }
}
