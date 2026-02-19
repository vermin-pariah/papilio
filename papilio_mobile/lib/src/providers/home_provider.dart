import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'dart:async';
import '../api/music_repository.dart';
import '../models/track.dart';
import '../models/album.dart';
import '../models/artist.dart';
import '../models/global_search_result.dart';

// Search Query Provider (autoDispose ensures cleanup when not in use)
final searchQueryProvider = StateProvider.autoDispose<String>((ref) => '');

// Global Search Provider
final globalSearchProvider = FutureProvider.autoDispose<GlobalSearchResult>((ref) async {
  final query = ref.watch(searchQueryProvider);
  if (query.isEmpty) return GlobalSearchResult(artists: [], albums: [], tracks: []);
  final repository = ref.watch(musicRepositoryProvider);
  return repository.globalSearch(query);
});

// Search History Provider
final searchHistoryProvider = StateNotifierProvider<SearchHistoryNotifier, List<String>>((ref) {
  return SearchHistoryNotifier();
});

// Dedicated Search Providers (Isolated from Home)
final searchArtistsProvider = FutureProvider.autoDispose<List<Artist>>((ref) async {
  final query = ref.watch(searchQueryProvider);
  if (query.isEmpty) return [];
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getArtists(query: query);
});

final searchAlbumsProvider = FutureProvider.autoDispose<List<Album>>((ref) async {
  final query = ref.watch(searchQueryProvider);
  if (query.isEmpty) return [];
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getAlbums(query: query);
});

final searchTracksProvider = FutureProvider.autoDispose<List<Track>>((ref) async {
  final query = ref.watch(searchQueryProvider);
  if (query.isEmpty) return [];
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getTracks(query: query, limit: 50);
});

class SearchHistoryNotifier extends StateNotifier<List<String>> {
  SearchHistoryNotifier() : super([]) {
    _load();
  }

  static const String _key = 'search_history';

  Future<void> _load() async {
    final prefs = await SharedPreferences.getInstance();
    state = prefs.getStringList(_key) ?? [];
  }

  Future<void> add(String query) async {
    if (query.trim().isEmpty) return;
    final List<String> newList = [query, ...state.where((item) => item != query)].take(10).toList();
    state = newList;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setStringList(_key, newList);
  }

  Future<void> clear() async {
    state = [];
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_key);
  }
}

// Artists & Albums (Global/Home versions)
final artistsProvider = FutureProvider<List<Artist>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getArtists();
});

final albumsProvider = FutureProvider<List<Album>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getAlbums();
});

// Paginated Tracks (Global/Home version)
final tracksProvider = AsyncNotifierProvider<PaginatedTracksNotifier, List<Track>>(() {
  return PaginatedTracksNotifier();
});

class PaginatedTracksNotifier extends AsyncNotifier<List<Track>> {
  int _offset = 0;
  final int _limit = 30;
  bool _hasMore = true;
  bool _isLoadingMore = false;

  bool get hasMore => _hasMore;

  @override
  FutureOr<List<Track>> build() async {
    _offset = 0;
    _hasMore = true;
    _isLoadingMore = false;
    return _fetchTracks();
  }

  Future<List<Track>> _fetchTracks() async {
    final repository = ref.read(musicRepositoryProvider);
    final results = await repository.getTracks(
      limit: _limit,
      offset: _offset,
    );
    if (results.length < _limit) _hasMore = false;
    return results;
  }

  Future<void> loadMore() async {
    if (_isLoadingMore || !_hasMore) return;
    _isLoadingMore = true;
    try {
      _offset += _limit;
      final newTracks = await _fetchTracks();
      final currentTracks = state.value ?? [];
      state = AsyncValue.data([...currentTracks, ...newTracks]);
    } catch (e, st) {
      state = AsyncError(e, st);
    } finally {
      _isLoadingMore = false;
    }
  }
}

final historyProvider = FutureProvider<List<Track>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getRecentHistory();
});

final favoritesProvider = FutureProvider<List<Track>>((ref) async {
  final repository = ref.watch(musicRepositoryProvider);
  return repository.getFavorites();
});