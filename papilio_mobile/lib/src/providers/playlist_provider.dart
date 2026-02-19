import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../api/music_repository.dart';
import '../models/track.dart';
import '../models/album.dart';

// User's Playlists Provider
final playlistsProvider = FutureProvider<List<Album>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getPlaylists();
});

// Favorites Preview Provider
final favoritesPreviewProvider = FutureProvider<List<Track>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getFavorites();
});

// History Preview Provider
final historyPreviewProvider = FutureProvider<List<Track>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getRecentHistory();
});

// 核心改进：播放列表歌曲 Provider
final playlistTracksProvider = FutureProvider.family<List<Track>, String>((ref, playlistId) async {
  final repository = ref.watch(musicRepositoryProvider);
  if (playlistId == 'favorites') return repository.getFavorites();
  if (playlistId == 'history') return repository.getRecentHistory();
  return repository.getPlaylistTracks(playlistId);
});

// 新增：专辑歌曲 Provider
final albumTracksProvider = FutureProvider.family<List<Track>, String>((ref, albumId) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getAlbumTracks(albumId);
});

// Playlist Controller
final playlistControllerProvider = Provider((ref) {
  final repository = ref.watch(musicRepositoryProvider);
  return PlaylistController(repository, ref);
});

class PlaylistController {
  final MusicRepository _repository;
  final ProviderRef _ref;

  PlaylistController(this._repository, this._ref);

  Future<void> create(String name, {String? description}) async {
    await _repository.createPlaylist(name, description: description);
    _ref.invalidate(playlistsProvider);
  }

  Future<void> update(String playlistId, String name, {String? description}) async {
    await _repository.updatePlaylist(playlistId, name, description: description);
    _ref.invalidate(playlistsProvider);
  }

  Future<void> addTrack(String playlistId, String trackId) async {
    await _repository.addTrackToPlaylist(playlistId, trackId);
    _ref.invalidate(playlistsProvider);
    _ref.invalidate(playlistTracksProvider(playlistId));
  }

  Future<void> addTracks(String playlistId, List<String> trackIds) async {
    for (final id in trackIds) {
      await _repository.addTrackToPlaylist(playlistId, id);
    }
    _ref.invalidate(playlistsProvider);
    _ref.invalidate(playlistTracksProvider(playlistId));
  }

  Future<void> removeTrack(String playlistId, String trackId) async {
    await _repository.removeTrackFromPlaylist(playlistId, trackId);
    _ref.invalidate(playlistsProvider);
    _ref.invalidate(playlistTracksProvider(playlistId));
  }

  Future<void> delete(String playlistId) async {
    await _repository.deletePlaylist(playlistId);
    _ref.invalidate(playlistsProvider);
    _ref.invalidate(playlistTracksProvider(playlistId));
  }

  Future<List<Track>> getTracks(String playlistId) async {
    return _repository.getPlaylistTracks(playlistId);
  }
}
