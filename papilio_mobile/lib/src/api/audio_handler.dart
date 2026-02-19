import 'package:audio_service/audio_service.dart';
import 'package:just_audio/just_audio.dart';
import 'package:audio_session/audio_session.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:path_provider/path_provider.dart';
import 'package:flutter/foundation.dart';
import 'dart:io';
import '../models/track.dart';
import '../config/env_config.dart';
import 'music_repository.dart';

class AudioPlayerHandler extends BaseAudioHandler with SeekHandler {
  final AudioPlayer _player = AudioPlayer();
  final _playlist = ConcatenatingAudioSource(children: []);
  MusicRepository? _repository;
  final List<String> _pendingRecordTasks = [];

  void setRepository(MusicRepository repo) {
    _repository = repo;
    // Process any tasks that were queued before the repository was ready
    if (_pendingRecordTasks.isNotEmpty) {
      for (var trackId in _pendingRecordTasks) {
        _repository?.recordPlay(trackId);
      }
      _pendingRecordTasks.clear();
    }
  }

  AudioPlayerHandler() {
    _initSession();
    _player.setAudioSource(_playlist);
    
    // Listen to ALL relevant streams and trigger a fresh state build
    _player.playbackEventStream.listen((_) => _updateState());
    _player.loopModeStream.listen((_) => _updateState());
    _player.shuffleModeEnabledStream.listen((_) => _updateState());
    _player.playingStream.listen((_) => _updateState());

    _player.currentIndexStream.listen((index) {
      if (index != null && index < _playlist.length) {
        final source = _playlist.children[index] as IndexedAudioSource;
        final item = source.tag as MediaItem;
        mediaItem.add(item);
        _persistLastTrack(item.id, index);
        
        // Auto-record play history with fallback queue
        if (_repository != null) {
          _repository!.recordPlay(item.id);
        } else {
          _pendingRecordTasks.add(item.id);
        }
      }
    });

    _player.positionStream.listen((pos) {
      if (_player.playing) {
        _persistPosition(pos);
        // Only update if not already being updated by pipe
        if (!playbackState.hasListener || _player.processingState == ProcessingState.ready) {
           // We can rely on _transformEvent which is piped
        }
      }
    });
  }

  Future<void> _initSession() async {
    final session = await AudioSession.instance;
    await session.configure(const AudioSessionConfiguration.music());
    
    session.becomingNoisyEventStream.listen((_) => pause());
    
    session.interruptionEventStream.listen((event) {
      if (event.begin) {
        switch (event.type) {
          case AudioInterruptionType.pause:
          case AudioInterruptionType.unknown:
            pause();
          case AudioInterruptionType.duck:
            _player.setVolume(0.5);
        }
      } else {
        if (event.type == AudioInterruptionType.duck) {
          _player.setVolume(1.0);
        } else if (event.type == AudioInterruptionType.pause) {
          play();
        }
      }
    });
  }

  Future<void> _persistLastTrack(String trackId, int index) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('last_track_id', trackId);
    await prefs.setInt('last_track_index', index);
  }

  Future<void> _persistPosition(Duration pos) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt('last_position_ms', pos.inMilliseconds);
  }

  void _updateState() {
    if (playbackState.hasListener) {
      playbackState.add(_buildPlaybackState());
    }
  }

  PlaybackState _buildPlaybackState() {
    return PlaybackState(
      controls: [
        MediaControl.skipToPrevious,
        if (_player.playing) MediaControl.pause else MediaControl.play,
        MediaControl.skipToNext,
        MediaControl.stop,
      ],
      systemActions: const {
        MediaAction.seek, 
        MediaAction.seekForward, 
        MediaAction.seekBackward,
        MediaAction.skipToNext,
        MediaAction.skipToPrevious,
        MediaAction.setRepeatMode,
        MediaAction.setShuffleMode,
      },
      androidCompactActionIndices: const [0, 1, 2],
      processingState: const {
        ProcessingState.idle: AudioProcessingState.idle,
        ProcessingState.loading: AudioProcessingState.loading,
        ProcessingState.buffering: AudioProcessingState.buffering,
        ProcessingState.ready: AudioProcessingState.ready,
        ProcessingState.completed: AudioProcessingState.completed,
      }[_player.processingState] ?? AudioProcessingState.idle,
      playing: _player.playing,
      updatePosition: _player.position,
      bufferedPosition: _player.bufferedPosition,
      speed: _player.speed,
      queueIndex: _player.currentIndex,
      repeatMode: switch (_player.loopMode) {
        LoopMode.off => AudioServiceRepeatMode.none,
        LoopMode.one => AudioServiceRepeatMode.one,
        LoopMode.all => AudioServiceRepeatMode.all,
      },
      shuffleMode: _player.shuffleModeEnabled ? AudioServiceShuffleMode.all : AudioServiceShuffleMode.none,
    );
  }

  @override
  Future<void> play() => _player.play();
  @override
  Future<void> pause() => _player.pause();
  @override
  Future<void> seek(Duration position) => _player.seek(position);
  @override
  Future<void> stop() => _player.stop();
  @override
  Future<void> skipToNext() async {
    final currentMode = _player.loopMode;
    try {
      if (currentMode == LoopMode.one) {
        await _player.setLoopMode(LoopMode.off);
      }
      await _player.seekToNext();
    } finally {
      if (currentMode == LoopMode.one) {
        await _player.setLoopMode(LoopMode.one);
      }
      if (!_player.playing) play();
    }
  }

  @override
  Future<void> skipToPrevious() async {
    final currentMode = _player.loopMode;
    try {
      if (currentMode == LoopMode.one) {
        await _player.setLoopMode(LoopMode.off);
      }
      await _player.seekToPrevious();
    } finally {
      if (currentMode == LoopMode.one) {
        await _player.setLoopMode(LoopMode.one);
      }
      if (!_player.playing) play();
    }
  }
  @override
  Future<void> skipToQueueItem(int index) => _player.seek(Duration.zero, index: index);

  @override
  Future<void> setRepeatMode(AudioServiceRepeatMode repeatMode) async {
    final loopMode = switch (repeatMode) {
      AudioServiceRepeatMode.none => LoopMode.off,
      AudioServiceRepeatMode.one => LoopMode.one,
      AudioServiceRepeatMode.all => LoopMode.all,
      AudioServiceRepeatMode.group => LoopMode.all,
    };
    await _player.setLoopMode(loopMode);
  }

  @override
  Future<void> setShuffleMode(AudioServiceShuffleMode shuffleMode) async {
    await _player.setShuffleModeEnabled(shuffleMode != AudioServiceShuffleMode.none);
  }

  int _requestTag = 0;

  Future<void> setQueue(List<Track> tracks, int initialIndex, EnvConfig config) async {
    final currentTag = ++_requestTag;

    try {
      final docDir = await getApplicationDocumentsDirectory();
      final downloadPath = "${docDir.path}/downloads";
      final List<AudioSource> audioSources = [];
      
      for (var track in tracks) {
        // Optimistic check: if a new request started, abort this loop immediately
        if (currentTag != _requestTag) return;

        final item = MediaItem(
          id: track.id,
          album: track.albumTitle ?? 'Unknown Album',
          title: track.title,
          artist: track.artistName ?? 'Unknown Artist',
          duration: Duration(seconds: track.duration),
          artUri: track.albumId != null ? Uri.parse('${config.coversBaseUrl}${track.albumId}') : null,
          extras: {
            'format': track.format, 
            'albumId': track.albumId, 
            'artistId': track.artistId,
            'artistName': track.artistName,
            'artistImageUrl': track.artistImageUrl,
            'albumTitle': track.albumTitle,
            'isFavorite': track.isFavorite,
            'lyrics': track.lyrics,
          },
        );

        final localFile = File("$downloadPath/${track.id}.audio");
        if (await localFile.exists()) {
          audioSources.add(AudioSource.uri(localFile.uri, tag: item));
        } else {
          final streamUrl = config.isDataSaverMode 
              ? "${config.streamBaseUrl}${track.id}?bitrate=128k"
              : "${config.streamBaseUrl}${track.id}";
          audioSources.add(LockCachingAudioSource(Uri.parse(streamUrl), tag: item));
        }
      }

      // Final version check before modifying the actual player state
      if (currentTag != _requestTag) return;
      
      await _player.stop();
      await _playlist.clear();
      await _playlist.addAll(audioSources);

      // Sync queue to AudioService
      queue.add(audioSources.map((s) => (s as IndexedAudioSource).tag as MediaItem).toList());
      
      if (audioSources.isNotEmpty) {
        final safeIndex = (initialIndex >= 0 && initialIndex < audioSources.length) ? initialIndex : 0;
        await _player.seek(Duration.zero, index: safeIndex);
        
        final source = audioSources[safeIndex] as IndexedAudioSource;
        mediaItem.add(source.tag as MediaItem);
        
        _player.play();
      }
    } catch (e) {
      debugPrint('Error setting queue: $e');
    }
  }

    Future<void> playTrack(Track track, EnvConfig config) async {

      await setQueue([track], 0, config);

    }

  }

  