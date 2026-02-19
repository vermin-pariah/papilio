import 'dart:ui';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:animate_do/animate_do.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../providers/home_provider.dart';
import '../providers/player_provider.dart';
import '../providers/scan_provider.dart';
import '../providers/download_provider.dart';
import '../providers/navigation_provider.dart';
import '../api/music_repository.dart';
import '../config/env_config.dart';
import '../models/track.dart';
import '../models/album.dart';
import '../models/scan_status.dart';
import 'playlist_view.dart';
import 'music_detail_view.dart';
import 'artist_detail_view.dart';
import 'widgets/track_action_sheet.dart';
import 'widgets/playing_visualizer.dart';

class HomeView extends ConsumerStatefulWidget {
  const HomeView({super.key});

  @override
  ConsumerState<HomeView> createState() => _HomeViewState();
}

class _HomeViewState extends ConsumerState<HomeView> {
  final ScrollController _scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (_scrollController.position.pixels >= _scrollController.position.maxScrollExtent - 200) {
      ref.read(tracksProvider.notifier).loadMore();
    }
  }

  @override
  Widget build(BuildContext context) {
    final tracksAsync = ref.watch(tracksProvider);
    final albumsAsync = ref.watch(albumsProvider);
    final historyAsync = ref.watch(historyProvider);
    final searchQuery = ref.watch(searchQueryProvider);
    final scanStatus = ref.watch(scanStatusProvider).value;

    return Scaffold(
      backgroundColor: Colors.transparent,
      body: RefreshIndicator(
        onRefresh: () async {
          ref.invalidate(tracksProvider);
          ref.invalidate(albumsProvider);
          ref.invalidate(historyProvider);
          await ref.read(tracksProvider.future);
        },
        child: CustomScrollView(
          controller: _scrollController,
          physics: const AlwaysScrollableScrollPhysics(parent: BouncingScrollPhysics()),
          slivers: [
                                                                    SliverAppBar(
                                                                      floating: true, pinned: true,
                                                                      centerTitle: false,
                                                                      elevation: 0,
                                                                      scrolledUnderElevation: 0,
                                                                      backgroundColor: const Color(0xFF0F172A), // 深海蓝，坚固的视觉背衬
                                                                      title: Text(
                                                                        'PAPILIO', 
                                                                        style: GoogleFonts.montserrat(
                                                                          fontWeight: FontWeight.w900, 
                                                                          fontSize: 20, 
                                                                          letterSpacing: 6, // 增加字间距提升工业感
                                                                          color: Colors.white,
                                                                        )
                                                                      ),
                                                                      actions: [
                                                                        IconButton(
                                                                          icon: const Icon(Icons.library_music_rounded, color: Colors.white70),
                                                                          onPressed: () => ref.read(navigationIndexProvider.notifier).state = 3,
                                                                        ),
                                                                        const SizedBox(width: 8),
                                                                      ],
                                                                    ),              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(20, 8, 20, 16),
                  child: TextField(
                    readOnly: true, // Make it an entry point
                    onTap: () {
                      // Navigate to search tab/page
                      ref.read(navigationIndexProvider.notifier).state = 1; // Assuming 1 is Search
                    },
                    decoration: InputDecoration(
                      hintText: '搜索曲目、专辑或艺术家...', 
                      prefixIcon: const Icon(Icons.search_rounded), 
                      filled: true, 
                      fillColor: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.5), 
                      border: OutlineInputBorder(borderRadius: BorderRadius.circular(20), borderSide: BorderSide.none)
                    ),
                  ),
                ),
              ),

              // Recently Played
              historyAsync.when(
                data: (h) => h.isEmpty ? const SliverToBoxAdapter(child: SizedBox()) : SliverToBoxAdapter(child: _HorizontalSection(title: '最近播放', child: SizedBox(height: 160, child: ListView.builder(scrollDirection: Axis.horizontal, padding: const EdgeInsets.symmetric(horizontal: 16), itemCount: h.length, itemBuilder: (c, i) => _SmallTrackCard(track: h[i], onTap: () => ref.read(playerControllerProvider)?.playQueue(h, i)))))),
                loading: () => const SliverToBoxAdapter(child: SizedBox(height: 100)),
                error: (e, _) => SliverToBoxAdapter(child: ListTile(title: Text('历史记录加载失败: $e', style: const TextStyle(color: Colors.redAccent, fontSize: 12)))),
              ),

              // Featured Albums
              albumsAsync.when(
                data: (a) => SliverToBoxAdapter(child: _HorizontalSection(title: '精选专辑', child: SizedBox(height: 220, child: ListView.builder(scrollDirection: Axis.horizontal, padding: const EdgeInsets.symmetric(horizontal: 16), itemCount: a.length, itemBuilder: (c, i) => _AlbumCard(album: a[i]))))),
                loading: () => const SliverToBoxAdapter(child: SizedBox(height: 150)),
                error: (e, _) => SliverToBoxAdapter(child: ListTile(title: Text('专辑加载失败: $e', style: const TextStyle(color: Colors.redAccent, fontSize: 12)))),
              ),

              SliverToBoxAdapter(child: Padding(padding: const EdgeInsets.fromLTRB(20, 32, 20, 16), child: Text('你的曲库', style: Theme.of(context).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.bold)))),

              // Main Track List
              tracksAsync.when(
                data: (tracks) => SliverList(delegate: SliverChildBuilderDelegate((c, i) {
                  if (i == tracks.length) return ref.watch(tracksProvider.notifier).hasMore ? const _BottomLoadingIndicator() : const SizedBox(height: 40, child: Center(child: Text('没有更多了', style: TextStyle(color: Colors.white24))));
                  return FadeInUp(delay: Duration(milliseconds: (i % 10) * 30), child: _TrackListTile(track: tracks[i], onTap: () => ref.read(playerControllerProvider)?.playQueue(tracks, i)));
                }, childCount: tracks.length + 1)),
                loading: () => const SliverFillRemaining(child: Center(child: CircularProgressIndicator())),
                error: (e, _) => SliverFillRemaining(child: _ErrorPlaceholder(
                  message: '曲库加载失败: $e', 
                  onRetry: () => ref.invalidate(tracksProvider)
                )),
              ),
              const SliverToBoxAdapter(child: SizedBox(height: 120)),
            ],
          ),
        ),
    );
  }
}

class _ScanProgressBanner extends StatelessWidget {
  final ScanStatus status;
  const _ScanProgressBanner({required this.status});
  @override
  Widget build(BuildContext context) {
    return Container(padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12), color: Theme.of(context).colorScheme.primaryContainer.withOpacity(0.3), child: Column(children: [
      Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [const Text('正在同步曲库...', style: TextStyle(fontWeight: FontWeight.bold, fontSize: 12)), Text('${(status.progress * 100).toInt()}% (${status.currentCount}/${status.totalCount})', style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold))]),
      const SizedBox(height: 8),
      LinearProgressIndicator(value: status.progress, backgroundColor: Colors.white10, borderRadius: BorderRadius.circular(4)),
    ]));
  }
}

class _BottomLoadingIndicator extends StatelessWidget {
  const _BottomLoadingIndicator();
  @override
  Widget build(BuildContext context) { return const Padding(padding: EdgeInsets.symmetric(vertical: 20), child: Center(child: SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2)))); }
}

class _HorizontalSection extends StatelessWidget {
  final String title; final Widget child;
  const _HorizontalSection({required this.title, required this.child});
  @override
  Widget build(BuildContext context) { return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [Padding(padding: const EdgeInsets.fromLTRB(20, 24, 20, 12), child: Text(title, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.bold))), child]); }
}

class _TrackListTile extends ConsumerWidget {
  final Track track; final VoidCallback onTap;
  const _TrackListTile({required this.track, required this.onTap});
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final config = ref.watch(envConfigNotifierProvider);
    final currentTrack = ref.watch(currentTrackProvider).value;
    final isPlaying = currentTrack?.id == track.id;
    final theme = Theme.of(context);
    
    // 监听下载状态
    final downloadState = ref.watch(downloadProvider);
    final downloadProgress = downloadState.progress[track.id];
    
    Widget? downloadStatus;
    if (downloadProgress != null) {
      downloadStatus = downloadProgress < 1.0 
        ? SizedBox(width: 16, height: 16, child: CircularProgressIndicator(value: downloadProgress, strokeWidth: 2))
        : const Icon(Icons.check_circle_outline_rounded, size: 18, color: Colors.greenAccent);
    }

    final coverUrl = track.albumId != null ? '${config.coversBaseUrl}${track.albumId}' : null;
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
      leading: Stack(
        children: [
          Container(width: 50, height: 50, decoration: BoxDecoration(borderRadius: BorderRadius.circular(8), color: theme.colorScheme.surfaceVariant), child: coverUrl != null ? ClipRRect(borderRadius: BorderRadius.circular(8), child: CachedNetworkImage(imageUrl: coverUrl, fit: BoxFit.cover, placeholder: (c, u) => Icon(Icons.music_note_rounded, color: Colors.white.withOpacity(0.2)), errorWidget: (c, u, e) => const Icon(Icons.music_note_rounded))) : const Icon(Icons.music_note_rounded)),
          if (isPlaying)
            Container(
              width: 50, height: 50,
              decoration: BoxDecoration(borderRadius: BorderRadius.circular(8), color: Colors.black38),
              child: Center(child: PlayingVisualizer(color: theme.colorScheme.primary, size: 18)),
            ),
        ],
      ),
      title: Text(
        track.title, 
        style: TextStyle(
          fontWeight: isPlaying ? FontWeight.bold : FontWeight.w600,
          color: isPlaying ? theme.colorScheme.primary : null,
        ), 
        maxLines: 1, overflow: TextOverflow.ellipsis
      ),
      subtitle: Text('${track.artistName ?? "未知艺术家"} • ${track.albumTitle ?? "未知专辑"}', style: const TextStyle(fontSize: 12), maxLines: 1, overflow: TextOverflow.ellipsis),
      trailing: Container(
        constraints: const BoxConstraints(maxWidth: 100),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            if (downloadStatus != null) ...[
              downloadStatus,
              const SizedBox(width: 8),
            ],
            IconButton(icon: const Icon(Icons.more_vert_rounded), onPressed: () => showTrackActionSheet(context, ref, track)),
          ],
        ),
      ),
      onTap: onTap,
    );
  }
}

class _SmallTrackCard extends ConsumerWidget {
  final Track track; final VoidCallback onTap;
  const _SmallTrackCard({required this.track, required this.onTap});
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final config = ref.watch(envConfigNotifierProvider);
    final coverUrl = track.albumId != null ? '${config.coversBaseUrl}${track.albumId}' : null;
    
    return GestureDetector(onTap: onTap, child: Container(width: 110, margin: const EdgeInsets.only(right: 14), child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Container(
        height: 100, 
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(16), 
          color: Theme.of(context).colorScheme.primaryContainer.withOpacity(0.5)
        ), 
        child: ClipRRect(
          borderRadius: BorderRadius.circular(16),
          child: coverUrl != null 
            ? CachedNetworkImage(imageUrl: coverUrl, fit: BoxFit.cover, placeholder: (c, u) => const Center(child: Icon(Icons.play_circle_fill_rounded, size: 40)), errorWidget: (c, u, e) => const Center(child: Icon(Icons.play_circle_fill_rounded, size: 40)))
            : const Center(child: Icon(Icons.play_circle_fill_rounded, size: 40)),
        )
      ),
      const SizedBox(height: 8),
      Text(track.title, maxLines: 2, overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w500)),
    ])));
  }
}

class _AlbumCard extends ConsumerWidget {
  final Album album;
  const _AlbumCard({required this.album});
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final config = ref.watch(envConfigNotifierProvider);
    final coverUrl = '${config.coversBaseUrl}${album.id}';
    return GestureDetector(
      onTap: () => Navigator.push(context, MaterialPageRoute(builder: (context) => MusicDetailView(item: album))),
      child: Container(width: 160, margin: const EdgeInsets.only(right: 16), child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Container(height: 160, decoration: BoxDecoration(borderRadius: BorderRadius.circular(24), boxShadow: [BoxShadow(color: Colors.black.withOpacity(0.1), blurRadius: 10, offset: const Offset(0, 5))], color: Theme.of(context).colorScheme.secondaryContainer), child: ClipRRect(borderRadius: BorderRadius.circular(24), child: CachedNetworkImage(imageUrl: coverUrl, fit: BoxFit.cover, placeholder: (c, u) => Center(child: Icon(Icons.album_rounded, size: 60, color: Colors.white.withOpacity(0.1))), errorWidget: (c, u, e) => const Center(child: Icon(Icons.album_rounded, size: 60))))),
        const SizedBox(height: 10),
        Text(album.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontWeight: FontWeight.bold)),
        Text('${album.releaseYear ?? "未知年份"}', style: TextStyle(fontSize: 12, color: Theme.of(context).colorScheme.outline)),
      ])),
    );
  }
}

class _ErrorPlaceholder extends StatelessWidget {
  final String message; final VoidCallback onRetry;
  const _ErrorPlaceholder({required this.message, required this.onRetry});
  @override
  Widget build(BuildContext context) {
    return Center(child: Padding(padding: const EdgeInsets.all(32), child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
      Icon(Icons.error_outline_rounded, size: 64, color: Theme.of(context).colorScheme.error),
      const SizedBox(height: 16),
      SelectableText(
        message.replaceAll('AppException: ', ''), 
        textAlign: TextAlign.center, 
        style: const TextStyle(fontSize: 14, color: Colors.white70, fontFamily: 'monospace')
      ),
      const SizedBox(height: 24),
      FilledButton.icon(onPressed: onRetry, icon: const Icon(Icons.refresh_rounded), label: const Text('点击重试')),
    ])));
  }
}