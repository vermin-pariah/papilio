import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:animate_do/animate_do.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../providers/home_provider.dart';
import '../providers/player_provider.dart';
import '../config/env_config.dart';
import 'music_detail_view.dart';
import 'artist_detail_view.dart';
import 'widgets/playing_visualizer.dart';
import 'widgets/track_action_sheet.dart';

class SearchView extends ConsumerStatefulWidget {
  const SearchView({super.key});

  @override
  ConsumerState<SearchView> createState() => _SearchViewState();
}

class _SearchViewState extends ConsumerState<SearchView> {
  late TextEditingController _controller;
  Timer? _debounce;

  @override
  void initState() {
    super.initState();
    final initialQuery = ref.read(searchQueryProvider);
    _controller = TextEditingController(text: initialQuery);
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _controller.dispose();
    // Reset search query when leaving the view to avoid polluting other views
    Future.microtask(() {
      if (mounted) ref.read(searchQueryProvider.notifier).state = '';
    });
    super.dispose();
  }

  void _onSearchChanged(String query) {
    // Instant UI feedback for the clear button visibility
    setState(() {}); 

    if (_debounce?.isActive ?? false) _debounce!.cancel();
    _debounce = Timer(const Duration(milliseconds: 500), () {
      if (mounted) {
        ref.read(searchQueryProvider.notifier).state = query;
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    // We listen to the provider to handle history taps or external resets
    ref.listen(searchQueryProvider, (prev, next) {
      if (next != _controller.text) {
        _controller.text = next;
        setState(() {});
      }
    });

    final query = ref.watch(searchQueryProvider);

    return DefaultTabController(
      length: 4,
      initialIndex: 0,
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          scrolledUnderElevation: 0,
          toolbarHeight: 100,
          title: Padding(
            padding: const EdgeInsets.only(top: 16),
            child: TextField(
              controller: _controller,
              autofocus: false,
              onChanged: _onSearchChanged,
              onSubmitted: (val) {
                _debounce?.cancel();
                ref.read(searchQueryProvider.notifier).state = val;
                ref.read(searchHistoryProvider.notifier).add(val);
              },
              decoration: InputDecoration(
                hintText: '搜索万物...',
                prefixIcon: const Icon(Icons.search_rounded),
                filled: true,
                fillColor: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.5),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(20),
                  borderSide: BorderSide.none,
                ),
                suffixIcon: _controller.text.isNotEmpty 
                  ? IconButton(
                      icon: const Icon(Icons.clear_rounded),
                      onPressed: () {
                        _controller.clear();
                        ref.read(searchQueryProvider.notifier).state = '';
                      },
                    )
                  : null,
              ),
            ),
          ),
          bottom: query.isEmpty ? null : const TabBar(
            isScrollable: true,
            tabAlignment: TabAlignment.start,
            tabs: [
              Tab(text: '综合'),
              Tab(text: '曲目'),
              Tab(text: '专辑'),
              Tab(text: '艺术家'),
            ],
          ),
        ),
        body: query.isEmpty ? _SearchContextView(
          onTapHistory: (val) {
            _controller.text = val;
            ref.read(searchQueryProvider.notifier).state = val;
          },
        ) : const TabBarView(
          children: [
            _GlobalSearchPage(),
            _TrackSearchList(),
            _AlbumSearchGrid(),
            _ArtistSearchList(),
          ],
        ),
      ),
    );
  }
}

class _GlobalSearchPage extends ConsumerWidget {
  const _GlobalSearchPage();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final resultAsync = ref.watch(globalSearchProvider);
    final config = ref.watch(envConfigNotifierProvider);

    return resultAsync.when(
      data: (res) {
        if (res.artists.isEmpty && res.albums.isEmpty && res.tracks.isEmpty) {
          return const Center(child: Text('没有找到相关结果'));
        }

        return ListView(
          padding: const EdgeInsets.all(20),
          children: [
            if (res.artists.isNotEmpty) ...[
              const Text('最佳匹配', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
              const SizedBox(height: 16),
              GestureDetector(
                onTap: () => Navigator.push(context, MaterialPageRoute(builder: (c) => ArtistDetailView(artist: res.artists.first))),
                child: Container(
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.3),
                    borderRadius: BorderRadius.circular(20),
                  ),
                  child: Row(
                    children: [
                      CircleAvatar(
                        radius: 30, 
                        backgroundColor: Colors.white10,
                        backgroundImage: config.getEffectiveImageUrl(res.artists.first.imageUrl) != null 
                          ? CachedNetworkImageProvider(config.getEffectiveImageUrl(res.artists.first.imageUrl)!) as ImageProvider
                          : null,
                        child: config.getEffectiveImageUrl(res.artists.first.imageUrl) == null ? const Icon(Icons.person_rounded, size: 30) : null,
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(res.artists.first.name, style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold)),
                            const Text('艺术家', style: TextStyle(fontSize: 14, color: Colors.white54)),
                          ],
                        ),
                      ),
                      const Icon(Icons.chevron_right_rounded),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 32),
            ],

            if (res.tracks.isNotEmpty) ...[
              const Text('单曲', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
              const SizedBox(height: 12),
              ...res.tracks.take(5).map((t) {
                final coverUrl = t.albumId != null ? '${config.coversBaseUrl}${t.albumId}' : null;
                return ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: ClipRRect(
                    borderRadius: BorderRadius.circular(8),
                    child: coverUrl != null 
                      ? CachedNetworkImage(imageUrl: coverUrl, width: 40, height: 40, fit: BoxFit.cover)
                      : Container(width: 40, height: 40, color: Colors.white10),
                  ),
                  title: Text(t.title, style: const TextStyle(fontWeight: FontWeight.w500)),
                  subtitle: Text(t.artistName ?? '未知艺术家', style: const TextStyle(fontSize: 12)),
                  onTap: () => ref.read(playerControllerProvider)?.playTrack(t),
                );
              }),
              const SizedBox(height: 24),
            ],

            if (res.albums.isNotEmpty) ...[
              const Text('专辑', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
              const SizedBox(height: 16),
              SizedBox(
                height: 180,
                child: ListView.builder(
                  scrollDirection: Axis.horizontal,
                  itemCount: res.albums.length,
                  itemBuilder: (c, i) {
                    final album = res.albums[i];
                    return GestureDetector(
                      onTap: () => Navigator.push(context, MaterialPageRoute(builder: (c) => MusicDetailView(item: album))),
                      child: Container(
                        width: 140,
                        margin: const EdgeInsets.only(right: 16),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            ClipRRect(
                              borderRadius: BorderRadius.circular(12),
                              child: CachedNetworkImage(
                                imageUrl: '${config.coversBaseUrl}${album.id}',
                                width: 140, height: 140, fit: BoxFit.cover,
                                errorWidget: (c, u, e) => Container(color: Colors.white10, child: const Icon(Icons.album)),
                              ),
                            ),
                            const SizedBox(height: 8),
                            Text(album.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 13, fontWeight: FontWeight.bold)),
                          ],
                        ),
                      ),
                    );
                  },
                ),
              ),
            ],
          ],
        );
      },
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text('搜索失败: $e')),
    );
  }
}

class _SearchContextView extends ConsumerWidget {

  final Function(String) onTapHistory;

  const _SearchContextView({required this.onTapHistory});



  @override

  Widget build(BuildContext context, WidgetRef ref) {

    final history = ref.watch(searchHistoryProvider);

    final artistsAsync = ref.watch(artistsProvider); // This shows 'Hot' artists from global



    return ListView(

      padding: const EdgeInsets.all(24),

      children: [

        if (history.isNotEmpty) ...[

          Row(

            mainAxisAlignment: MainAxisAlignment.spaceBetween,

            children: [

              const Text('最近搜索', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),

              TextButton(

                onPressed: () => ref.read(searchHistoryProvider.notifier).clear(),

                child: const Text('清空'),

              ),

            ],

          ),

          const SizedBox(height: 12),

          Wrap(

            spacing: 10,

            children: history.map((item) => ActionChip(

              label: Text(item),

              onPressed: () => onTapHistory(item),

              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),

            )).toList(),

          ),

          const SizedBox(height: 40),

        ],



        const Text('热门发现', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),

        const SizedBox(height: 16),

        artistsAsync.when(

          data: (artists) => Wrap(

            spacing: 12, runSpacing: 12,

            children: artists.take(6).map((artist) => GestureDetector(

              onTap: () => Navigator.push(context, MaterialPageRoute(builder: (context) => ArtistDetailView(artist: artist))),

              child: Container(

                padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),

                decoration: BoxDecoration(

                  color: Theme.of(context).colorScheme.primaryContainer.withOpacity(0.3),

                  borderRadius: BorderRadius.circular(12),

                ),

                child: Text(artist.name, style: const TextStyle(fontWeight: FontWeight.w500)),

              ),

            )).toList(),

          ),

          loading: () => const Center(child: CircularProgressIndicator()),

          error: (_, __) => const SizedBox(),

        ),

      ],

    );

  }

}



class _TrackSearchList extends ConsumerWidget {

  const _TrackSearchList();



  @override

  Widget build(BuildContext context, WidgetRef ref) {

    final tracksAsync = ref.watch(searchTracksProvider);

    final config = ref.watch(envConfigNotifierProvider);



    return tracksAsync.when(

      data: (tracks) => ListView.builder(

        padding: const EdgeInsets.symmetric(vertical: 12),

        itemCount: tracks.length,

                itemBuilder: (context, index) {

                  final track = tracks[index];

                  final coverUrl = track.albumId != null ? '${config.coversBaseUrl}${track.albumId}' : null;

                  final currentTrack = ref.watch(currentTrackProvider).value;

                  final isPlaying = currentTrack?.id == track.id;

                  final theme = Theme.of(context);

                  

                  return ListTile(

                    contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),

                    leading: Stack(

                      children: [

                        ClipRRect(

                          borderRadius: BorderRadius.circular(8),

                          child: coverUrl != null 

                            ? CachedNetworkImage(

                                imageUrl: coverUrl, width: 48, height: 48, fit: BoxFit.cover,

                                placeholder: (_, __) => Container(color: Colors.white.withOpacity(0.1)),

                              )

                            : Container(color: Colors.white.withOpacity(0.1), width: 48, height: 48),

                        ),

                        if (isPlaying)

                          Container(

                            width: 48, height: 48,

                            decoration: BoxDecoration(borderRadius: BorderRadius.circular(8), color: Colors.black38),

                            child: Center(child: PlayingVisualizer(color: theme.colorScheme.primary, size: 18)),

                          ),

                      ],

                    ),

                    title: Text(

                      track.title, 

                      style: TextStyle(

                        fontWeight: isPlaying ? FontWeight.bold : FontWeight.bold,

                        color: isPlaying ? theme.colorScheme.primary : null,

                      )

                    ),

                    subtitle: Text(track.artistName ?? '未知艺术家', style: isPlaying ? TextStyle(color: theme.colorScheme.primary.withOpacity(0.7)) : null),

                    onTap: () {

              ref.read(searchHistoryProvider.notifier).add(ref.read(searchQueryProvider));

              ref.read(playerControllerProvider)?.playQueue(tracks, index);

            },

          );

        },

      ),

      loading: () => const Center(child: CircularProgressIndicator()),

      error: (e, _) => Center(child: Text('搜索失败: $e')),

    );

  }

}



class _AlbumSearchGrid extends ConsumerWidget {

  const _AlbumSearchGrid();



  @override

  Widget build(BuildContext context, WidgetRef ref) {

    final albumsAsync = ref.watch(searchAlbumsProvider);

    final config = ref.watch(envConfigNotifierProvider);



    return albumsAsync.when(

      data: (albums) => GridView.builder(

        padding: const EdgeInsets.all(20),

        gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(

          crossAxisCount: 2, mainAxisSpacing: 20, crossAxisSpacing: 20, childAspectRatio: 0.8

        ),

        itemCount: albums.length,

        itemBuilder: (context, index) {

          final album = albums[index];

          return GestureDetector(

            onTap: () {

              ref.read(searchHistoryProvider.notifier).add(ref.read(searchQueryProvider));

              Navigator.push(context, MaterialPageRoute(builder: (context) => MusicDetailView(item: album)));

            },

            child: Column(

              crossAxisAlignment: CrossAxisAlignment.start,

              children: [

                Expanded(

                  child: ClipRRect(

                    borderRadius: BorderRadius.circular(16),

                    child: CachedNetworkImage(

                      imageUrl: '${config.coversBaseUrl}${album.id}',

                      fit: BoxFit.cover,

                      placeholder: (_, __) => Container(color: Colors.white.withOpacity(0.1)),

                      errorWidget: (_, __, ___) => Container(color: Colors.white.withOpacity(0.1), child: const Icon(Icons.album)),

                    ),

                  ),

                ),

                const SizedBox(height: 8),

                Text(album.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontWeight: FontWeight.bold)),

              ],

            ),

          );

        },

      ),

      loading: () => const Center(child: CircularProgressIndicator()),

      error: (e, _) => Center(child: Text('搜索失败: $e')),

    );

  }

}



class _ArtistSearchList extends ConsumerWidget {

  const _ArtistSearchList();



  @override

  Widget build(BuildContext context, WidgetRef ref) {

    final artistsAsync = ref.watch(searchArtistsProvider);



        final config = ref.watch(envConfigNotifierProvider);



    



        return artistsAsync.when(



          data: (artists) => ListView.builder(



            padding: const EdgeInsets.symmetric(vertical: 12),



            itemCount: artists.length,



                        itemBuilder: (context, index) {



                                    final artist = artists[index];



                                    final effectiveImageUrl = config.getEffectiveImageUrl(artist.imageUrl);



                          



                                              return ListTile(



              



                                    leading: CircleAvatar(



              



                                      backgroundColor: Colors.white10,



              



                                      backgroundImage: effectiveImageUrl != null 



              



                                        ? CachedNetworkImageProvider(effectiveImageUrl) as ImageProvider



              



                                        : null,



              



                                      child: effectiveImageUrl == null ? const Icon(Icons.person_rounded, color: Colors.white38) : null,



              



                                    ),



              



                                    title: Text(artist.name, style: const TextStyle(fontWeight: FontWeight.bold)),



              



                                    subtitle: Text(effectiveImageUrl ?? 'No URL', style: const TextStyle(fontSize: 8, color: Colors.white38)),

            onTap: () {

              ref.read(searchHistoryProvider.notifier).add(ref.read(searchQueryProvider));

              Navigator.push(

                context,

                MaterialPageRoute(builder: (context) => ArtistDetailView(artist: artist)),

              );

            },

          );

        },

      ),

      loading: () => const Center(child: CircularProgressIndicator()),

      error: (e, _) => Center(child: Text('搜索失败: $e')),

    );

  }

}
