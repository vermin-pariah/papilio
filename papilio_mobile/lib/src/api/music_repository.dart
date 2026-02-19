import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:dio/dio.dart';
import '../models/track.dart';
import '../models/album.dart';
import '../models/artist.dart';
import '../models/scan_status.dart';
import '../models/artist_sync_status.dart';
import '../models/app_exception.dart';
import '../models/global_search_result.dart';
import 'api_client.dart';

part 'music_repository.g.dart';

@riverpod
MusicRepository musicRepository(MusicRepositoryRef ref) {
  final client = ref.watch(apiClientProvider);
  return MusicRepository(client);
}

class MusicRepository {
  final ApiClient _client;

  MusicRepository(this._client);

  Future<T> _safeRequest<T>(Future<Response> Function() call, T defaultValue) async {
    try {
      final response = await call();
      return response.data as T;
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return defaultValue;
      rethrow;
    }
  }

  Future<GlobalSearchResult> globalSearch(String query) async {
    try {
      final response = await _client.get('music/search', queryParameters: {'q': query});
      return GlobalSearchResult.fromJson(response.data);
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return GlobalSearchResult(tracks: [], albums: [], artists: []);
      rethrow;
    }
  }

  Future<List<Artist>> getArtists({String? query, int? limit, int? offset}) async {
    try {
      final response = await _client.get('music/artists', queryParameters: {
        if (query != null) 'q': query,
        if (limit != null) 'limit': limit,
        if (offset != null) 'offset': offset,
      });
      return (response.data as List).map((json) => Artist.fromJson(json)).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      rethrow;
    }
  }

  Future<Artist> getArtistById(String id) async {
    final artists = await getArtists();
    return artists.firstWhere((a) => a.id == id);
  }

  Future<List<Album>> getAlbums({String? query, int? limit, int? offset, String? artistId}) async {
    try {
      final response = await _client.get('music/albums', queryParameters: {
        if (query != null) 'q': query,
        if (limit != null) 'limit': limit,
        if (offset != null) 'offset': offset,
        if (artistId != null) 'artist_id': artistId,
      });
      return (response.data as List).map((json) => Album.fromJson(json)).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      rethrow;
    }
  }

  Future<Album> getAlbumById(String id) async {
    final albums = await getAlbums();
    return albums.firstWhere((a) => a.id == id);
  }

  Future<List<Track>> getTracks({String? query, int? limit, int? offset, String? albumId, String? artistId}) async {
    try {
      final response = await _client.get('music/tracks', queryParameters: {
        if (query != null) 'q': query,
        if (limit != null) 'limit': limit,
        if (offset != null) 'offset': offset,
        if (albumId != null) 'album_id': albumId,
        if (artistId != null) 'artist_id': artistId,
      });
      return (response.data as List).map((json) => Track.fromJson(json)).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      if (e.error is AppException) throw e.error as AppException;
      throw AppException(message: e.message ?? "未知错误");
    }
  }

  Future<Track?> getTrackById(String id) async {
    try {
      final response = await _client.get('music/tracks/$id');
      return Track.fromJson(response.data['track']);
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return null;
      rethrow;
    }
  }

  Future<List<Track>> getAlbumTracks(String albumId) => getTracks(albumId: albumId, limit: 1000);
  Future<List<Album>> getArtistAlbums(String artistId) => getAlbums(artistId: artistId);
  Future<List<Track>> getArtistTracks(String artistId) => getTracks(artistId: artistId, limit: 1000);

  Future<List<Track>> getFavorites() async {
    try {
      final response = await _client.get('music/favorites');
      return (response.data as List).map((json) => Track.fromJson(json)).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      rethrow;
    }
  }

  Future<bool> toggleFavorite(String trackId) async {
    final response = await _client.post('music/favorites/$trackId');
    return response.data['is_favorite'] ?? false;
  }

  Future<List<Track>> getRecentHistory() async {
    try {
      final response = await _client.get('music/history');
      return (response.data as List).map((json) => Track.fromJson(json)).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      rethrow;
    }
  }

  Future<void> recordPlay(String trackId) async {
    await _client.post('music/play/$trackId');
  }

  Future<void> updateCloudPlayback(String trackId, int positionMs) async {
    try {
      await _client.post('music/playback', data: {'track_id': trackId, 'position_ms': positionMs});
    } catch (_) {}
  }

  Future<Map<String, dynamic>?> getCloudPlayback() async {
    try {
      final response = await _client.get('music/playback');
      return response.data;
    } catch (_) { return null; }
  }

  Future<ScanStatus> getScanStatus() async {
    try {
      final response = await _client.get('music/scan/status');
      return ScanStatus.fromJson(response.data);
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return ScanStatus(isScanning: false, currentCount: 0, totalCount: 0);
      rethrow;
    }
  }

  Future<void> triggerScan() => _client.post('music/scan');

  Future<int> testConnection() async {
    final sw = Stopwatch()..start();
    try {
      await _client.get('music/tracks', queryParameters: {'limit': 1}); 
      return sw.elapsedMilliseconds;
    } catch (e) {
      return sw.elapsedMilliseconds;
    }
  }

  Future<void> downloadTrack(String trackId, String savePath, ProgressCallback onProgress) async {
    final downloadUrl = '${_client.apiBaseUrl}music/stream/$trackId';
    await _client.dioInstance.download(downloadUrl, savePath, onReceiveProgress: onProgress);
  }

  Future<List<Album>> getPlaylists() async {
    try {
      final response = await _client.get('playlists');
      return (response.data as List).map((json) => Album(
        id: json['id'],
        title: json['name'],
        artistId: json['user_id'],
        coverPath: null,
      )).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      rethrow;
    }
  }

  Future<void> createPlaylist(String name, {String? description}) async {
    await _client.post('playlists', data: {
      'name': name,
      'description': description,
      'is_public': false,
    });
  }

  Future<void> updatePlaylist(String playlistId, String name, {String? description}) async {
    await _client.patch('playlists/$playlistId', data: {
      'name': name,
      'description': description,
      'is_public': false,
    });
  }

  Future<void> addTrackToPlaylist(String playlistId, String trackId) async {
    await _client.post('playlists/$playlistId/tracks/$trackId');
  }

  Future<void> removeTrackFromPlaylist(String playlistId, String trackId) async {
    await _client.delete('playlists/$playlistId/tracks/$trackId');
  }

  Future<void> deletePlaylist(String playlistId) async {
    await _client.delete('playlists/$playlistId');
  }

  Future<List<Track>> getPlaylistTracks(String playlistId) async {
    try {
      final response = await _client.get('playlists/$playlistId');
      return (response.data['tracks'] as List).map((json) => Track.fromJson(json)).toList();
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return [];
      rethrow;
    }
  }

  Future<void> reorderPlaylistTracks(String playlistId, List<String> trackIds) async {
    await _client.post('playlists/$playlistId/reorder', data: trackIds);
  }

  Future<String?> getLyrics(String trackId) async {
    try {
      final response = await _client.get('music/lyrics/$trackId');
      return response.data.toString();
    } catch (e) {
      return null;
    }
  }

  Future<int> getLyricOffset(String trackId) async {
    try {
      final response = await _client.get('music/tracks/$trackId/lyric-offset');
      return response.data['offset_ms'] ?? 0;
    } catch (_) {
      return 0;
    }
  }

  Future<void> updateLyricOffset(String trackId, int offsetMs) async {
    await _client.post('music/tracks/$trackId/lyric-offset', data: {
      'offset_ms': offsetMs,
    });
  }

  Future<void> rescanMetadata(String trackId) async {
    await _client.post('music/tracks/$trackId/rescan');
  }

  // --- Admin APIs ---
  Future<Map<String, dynamic>> getAdminConfig() async {
    final response = await _client.get('admin/config');
    return response.data as Map<String, dynamic>;
  }

  Future<void> updateAdminConfig(String key, dynamic value) async {
    await _client.post('admin/config', data: {'key': key, 'value': value});
  }

  Future<Map<String, dynamic>> getSystemStatus() async {
    final response = await _client.get('admin/status');
    return response.data as Map<String, dynamic>;
  }

  Future<List<dynamic>> listUsers() async {
    final response = await _client.get('admin/users');
    return response.data as List<dynamic>;
  }

  Future<void> updateUserRole(String userId, bool isAdmin) async {
    await _client.post('admin/users/$userId/role', data: {'is_admin': isAdmin});
  }

  Future<void> deleteUser(String userId) async {
    await _client.delete('admin/users/$userId');
  }

  Future<ArtistSyncStatus> getArtistSyncStatus() async {
    final response = await _client.get('admin/sync-artists/status');
    return ArtistSyncStatus.fromJson(response.data);
  }

  Future<void> triggerArtistSync() async {
    await _client.post('admin/sync-artists');
  }

  Future<void> triggerArtistSyncMissing() async {
    await _client.post('admin/sync-artists/missing');
  }

  Future<void> triggerLibraryOrganize() async {
    await _client.post('admin/library/organize');
  }

  Future<void> triggerArtistSyncSingle(String artistId) async {
    await _client.post('admin/sync-artists/$artistId');
  }

  Future<void> uploadArtistAvatar(String artistId, String filePath) async {
    final formData = FormData.fromMap({
      'file': await MultipartFile.fromFile(filePath, filename: 'avatar.jpg'),
    });
    await _client.post('admin/artists/$artistId/avatar', data: formData);
  }
}
