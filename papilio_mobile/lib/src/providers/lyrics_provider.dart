import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:audio_service/audio_service.dart';
import '../api/music_repository.dart';
import '../models/lyric.dart';
import 'player_provider.dart';
import 'settings_provider.dart';

import '../config/env_config.dart';

// 获取当前曲目的原始歌词
final rawLyricsProvider = FutureProvider<List<LyricLine>?>((ref) async {
  final mediaItem = ref.watch(currentTrackProvider).value;
  if (mediaItem == null) return null;

  // 加载原始歌词
  final rawLrc = mediaItem.extras?['lyrics'] as String?;
  if (rawLrc != null && rawLrc.isNotEmpty) {
    return LyricLine.parse(rawLrc);
  }
  
  final repository = ref.watch(musicRepositoryProvider);
  final lrcContent = await repository.getLyrics(mediaItem.id);
  
  if (lrcContent == null || lrcContent.isEmpty) return null;
  return LyricLine.parse(lrcContent);
});

// 管理当前曲目的偏移
final songLyricOffsetProvider = StateNotifierProvider<SongOffsetNotifier, int>((ref) {
  final mediaItem = ref.watch(currentTrackProvider).value;
  final repository = ref.watch(musicRepositoryProvider);
  return SongOffsetNotifier(repository, mediaItem?.id);
});

class SongOffsetNotifier extends StateNotifier<int> {
  final MusicRepository _repository;
  final String? _trackId;
  SongOffsetNotifier(this._repository, this._trackId) : super(0) {
    if (_trackId != null) _loadOffset();
  }
  Future<void> _loadOffset() async {
    try {
      final offset = await _repository.getLyricOffset(_trackId!);
      state = offset;
    } catch (_) {}
  }

  Future<void> setOffset(int offsetMs) async {
    state = offsetMs;
    if (_trackId != null) {
      _repository.updateLyricOffset(_trackId!, offsetMs);
    }
  }

  Future<void> rescanTrack() async {
    if (_trackId == null) return;
    try {
      await _repository.rescanMetadata(_trackId!);
    } catch (_) {}
  }
}

// 降低刷新频率，歌词更新不需要 100ms
final _playbackTickerProvider = StreamProvider<void>((ref) {
  final playing = ref.watch(playbackStateProvider.select((s) => s.value?.playing ?? false));
  if (!playing) return Stream.value(null);
  return Stream.periodic(const Duration(milliseconds: 250));
});

// 计算当前歌词索引 (核心：回归物理真实，严禁状态缓存)
final currentLyricIndexProvider = Provider<int>((ref) {
  final lyrics = ref.watch(rawLyricsProvider).value;
  final playbackState = ref.watch(playbackStateProvider).value;
  final globalOffsetMs = ref.watch(settingsProvider).globalLyricOffsetMs;
  final songOffsetMs = ref.watch(songLyricOffsetProvider);

  if (lyrics == null || playbackState == null || lyrics.isEmpty) return -1;
  
  // 订阅 Ticker 以保持更新 (仅在播放时订阅)
  if (playbackState.playing) {
    ref.watch(_playbackTickerProvider);
  }

  // 核心修复：严禁使用 DateTime.now() 进行预测插值，直接信赖物理进度
  // 以前的逻辑：final elapsedMs = now.difference(lastUpdate).inMilliseconds; 
  // 这会导致预测超前，从而产生跳变。
  
  final positionMs = playbackState.position.inMilliseconds;

  final totalOffsetSeconds = (globalOffsetMs + songOffsetMs) / 1000.0;
  final effectivePosition = (positionMs / 1000.0) + totalOffsetSeconds;

  // 二分查找 (优化：从结果中筛选最接近的)
  int low = 0;
  int high = lyrics.length - 1;
  int result = -1;

  while (low <= high) {
    int mid = (low + high) ~/ 2;
    if (lyrics[mid].time <= effectivePosition) {
      result = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  
  return result;
});
