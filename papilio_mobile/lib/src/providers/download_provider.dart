import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'dart:io';
import 'dart:convert';
import '../api/music_repository.dart';
import '../models/track.dart';

class DownloadState {
  final Map<String, double> progress;
  final List<Track> downloading;
  final List<Track> completed;

  DownloadState({
    this.progress = const {},
    this.downloading = const [],
    this.completed = const [],
  });

  DownloadState copyWith({
    Map<String, double>? progress,
    List<Track>? downloading,
    List<Track>? completed,
  }) {
    return DownloadState(
      progress: progress ?? this.progress,
      downloading: downloading ?? this.downloading,
      completed: completed ?? this.completed,
    );
  }
}

final downloadProvider = StateNotifierProvider<DownloadNotifier, DownloadState>((ref) {
  final repository = ref.watch(musicRepositoryProvider);
  return DownloadNotifier(repository);
});

class DownloadNotifier extends StateNotifier<DownloadState> {
  final MusicRepository _repository;
  static const String _metadataKey = 'downloaded_metadata';
  
  DownloadNotifier(this._repository) : super(DownloadState()) {
    _loadLocalMetadata();
  }

  Future<String> _getDownloadDir() async {
    final dir = await getApplicationDocumentsDirectory();
    final path = "${dir.path}/downloads";
    final directory = Directory(path);
    if (!await directory.exists()) {
      await directory.create(recursive: true);
    }
    return path;
  }

  Future<void> _loadLocalMetadata() async {
    final prefs = await SharedPreferences.getInstance();
    final data = prefs.getStringList(_metadataKey);
    if (data != null) {
      final List<Track> tracks = data.map((j) => Track.fromJson(jsonDecode(j))).toList();
      state = state.copyWith(completed: tracks);
    }
    refreshCompleted();
  }

  Future<void> _saveLocalMetadata() async {
    final prefs = await SharedPreferences.getInstance();
    final list = state.completed.map((t) => jsonEncode(t.toJson())).toList();
    await prefs.setStringList(_metadataKey, list);
  }

  Future<void> refreshCompleted() async {
    final dirPath = await _getDownloadDir();
    final List<Track> validTracks = [];
    
    for (var track in state.completed) {
      if (await File("$dirPath/${track.id}.audio").exists()) {
        validTracks.add(track);
      }
    }
    
    if (validTracks.length != state.completed.length) {
      state = state.copyWith(completed: validTracks);
      _saveLocalMetadata();
    }
  }

  Future<double> getFileSize(String trackId) async {
    try {
      final dir = await _getDownloadDir();
      final file = File("$dir/$trackId.audio");
      if (await file.exists()) {
        final length = await file.length();
        return length / (1024 * 1024); // MB
      }
    } catch (_) {}
    return 0.0;
  }

  Future<void> download(Track track) async {
    if (state.downloading.any((t) => t.id == track.id)) return;

    final dir = await _getDownloadDir();
    final savePath = "$dir/${track.id}.audio";

    try {
      state = state.copyWith(
        downloading: [...state.downloading, track],
        progress: {...state.progress, track.id: 0.0},
      );

      await _repository.downloadTrack(track.id, savePath, (received, total) {
        if (total != -1) {
          state = state.copyWith(
            progress: {...state.progress, track.id: received / total},
          );
        }
      });

      state = state.copyWith(
        downloading: state.downloading.where((t) => t.id != track.id).toList(),
        completed: [...state.completed.where((t) => t.id != track.id), track],
        progress: {...state.progress, track.id: 1.0},
      );
      _saveLocalMetadata();
    } catch (e) {
      state = state.copyWith(
        downloading: state.downloading.where((t) => t.id != track.id).toList(),
        progress: Map.from(state.progress)..remove(track.id),
      );
      rethrow;
    }
  }

  Future<void> deleteDownload(String trackId) async {
    final dir = await _getDownloadDir();
    final file = File("$dir/$trackId.audio");
    if (await file.exists()) await file.delete();
    
    state = state.copyWith(
      completed: state.completed.where((t) => t.id != trackId).toList(),
    );
    _saveLocalMetadata();
  }

  Future<void> clearAllDownloads() async {
    final dirPath = await _getDownloadDir();
    final directory = Directory(dirPath);
    if (await directory.exists()) {
      await directory.list().forEach((f) {
        if (f is File && f.path.endsWith('.audio')) f.deleteSync();
      });
    }
    state = state.copyWith(completed: [], progress: {});
    _saveLocalMetadata();
  }

  Future<void> downloadAlbum(List<Track> tracks) async {
    for (var track in tracks) download(track);
  }
}
