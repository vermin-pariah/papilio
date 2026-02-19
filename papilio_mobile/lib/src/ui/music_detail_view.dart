import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../api/music_repository.dart';
import '../providers/player_provider.dart';
import '../providers/playlist_provider.dart';
import '../providers/download_provider.dart';
import '../config/env_config.dart';
import '../models/track.dart';
import '../models/album.dart';
import 'widgets/gradient_scaffold.dart';
import 'widgets/track_action_sheet.dart';
import 'widgets/playing_visualizer.dart';
import 'playlist_view.dart';

class MusicDetailView extends ConsumerStatefulWidget {
  final Album item; 
  final bool isPlaylist;
  final bool isFavorites;
  final bool isHistory;

  const MusicDetailView({
    super.key, 
    required this.item, 
    this.isPlaylist = false,
    this.isFavorites = false,
    this.isHistory = false,
  });

  @override
  ConsumerState<MusicDetailView> createState() => _MusicDetailViewState();
}

class _MusicDetailViewState extends ConsumerState<MusicDetailView> {
  // 移除本地 _tracks 状态，改为由 Provider 驱动
  
  Future<void> _handleReorder(List<Track> tracks, int oldIndex, int newIndex) async {
    final updatedTracks = List<Track>.from(tracks);
    if (oldIndex < newIndex) newIndex -= 1;
    final Track item = updatedTracks.removeAt(oldIndex);
    updatedTracks.insert(newIndex, item);
    
    try {
      final trackIds = updatedTracks.map((t) => t.id).toList();
      await ref.read(musicRepositoryProvider).reorderPlaylistTracks(widget.item.id, trackIds);
      ref.invalidate(playlistTracksProvider(widget.item.id));
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('同步排序失败')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final config = ref.watch(envConfigNotifierProvider);
    final playlistId = widget.isFavorites ? 'favorites' : (widget.isHistory ? 'history' : widget.item.id);
    
    // 核心修复：根据类型选择正确的 Provider
    final tracksAsync = (widget.isPlaylist || widget.isFavorites || widget.isHistory)
        ? ref.watch(playlistTracksProvider(playlistId))
        : ref.watch(albumTracksProvider(widget.item.id));
    
    final downloadStates = ref.watch(downloadProvider);

    return GradientScaffold(
      body: tracksAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('加载失败: $e')),
        data: (tracks) {
          // 封面逻辑
          String? effectiveCoverUrl;
          if ((widget.isFavorites || widget.isHistory || widget.isPlaylist) && tracks.isNotEmpty) {
            final trackWithCover = tracks.cast<Track?>().firstWhere((t) => t?.albumId != null, orElse: () => tracks.first);
            if (trackWithCover?.albumId != null) {
              effectiveCoverUrl = '${config.coversBaseUrl}${trackWithCover!.albumId}';
            }
          } else if (!widget.isFavorites && !widget.isHistory && !widget.isPlaylist) {
            effectiveCoverUrl = '${config.coversBaseUrl}${widget.item.id}';
          }

          // 仅当是真正的自定义列表时才传递 ID，以启用移除功能
          // 收藏和历史记录由其他逻辑处理（如 toggleFavorite）
          final tilePlaylistId = widget.isPlaylist ? widget.item.id : null;
          
          debugPrint('MusicDetailView: Rendering tracks. isPlaylist=${widget.isPlaylist}, itemId=${widget.item.id}, tilePlaylistId=$tilePlaylistId');

          return CustomScrollView(
            physics: const BouncingScrollPhysics(),
            slivers: [
              SliverAppBar(
                expandedHeight: 320, 
                pinned: true,
                stretch: true,
                backgroundColor: Theme.of(context).colorScheme.surface,
                flexibleSpace: FlexibleSpaceBar(
                  stretchModes: const [StretchMode.zoomBackground, StretchMode.blurBackground],
                  background: Stack(
                    fit: StackFit.expand,
                    children: [
                      if (effectiveCoverUrl != null)
                        CachedNetworkImage(
                          imageUrl: effectiveCoverUrl, fit: BoxFit.cover,
                          placeholder: (_, __) => _PlaceholderCover(),
                          errorWidget: (_, __, ___) => _PlaceholderCover(),
                        )
                      else
                        _PlaceholderCover(),
                      Container(
                        decoration: BoxDecoration(
                          gradient: LinearGradient(
                            colors: [Colors.black.withOpacity(0.8), Colors.transparent], 
                            begin: Alignment.bottomCenter, 
                            end: Alignment.topCenter
                          )
                        )
                      ),
                    ],
                  ),
                ),
              ),

              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 24),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 24),
                        child: Text(widget.item.title, style: Theme.of(context).textTheme.headlineMedium?.copyWith(fontWeight: FontWeight.bold)),
                      ),
                      const SizedBox(height: 8),
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 24),
                        child: Text(
                          widget.isFavorites 
                            ? '你的红心歌曲 • ${tracks.length} 首'
                            : widget.isHistory
                              ? '最近播放的足迹 • ${tracks.length} 首'
                              : widget.isPlaylist 
                                ? '你的播放列表 • ${tracks.length} 首' 
                                : '专辑 • ${widget.item.releaseYear ?? "未知年份"}', 
                          style: TextStyle(color: Colors.white.withOpacity(0.6))
                        ),
                      ),
                      const SizedBox(height: 24),
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 24),
                        child: Row( // 核心改进：Wrap 改为 Row，并使用 Flexible
                          children: [
                            // 播放全部 - 主按钮
                            Expanded(
                              flex: 3,
                              child: FilledButton.icon(
                                onPressed: () {
                                  if (tracks.isNotEmpty) {
                                    ref.read(playerControllerProvider)?.playQueue(tracks, 0);
                                  }
                                },
                                icon: const Icon(Icons.play_arrow_rounded),
                                label: const Text('播放'),
                                style: FilledButton.styleFrom(
                                  padding: const EdgeInsets.symmetric(vertical: 12),
                                ),
                              ),
                            ),
                            const SizedBox(width: 8),
                            // 下载
                            IconButton.filledTonal(
                              onPressed: () {
                                if (tracks.isNotEmpty) {
                                  ref.read(downloadProvider.notifier).downloadAlbum(tracks);
                                  ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('已加入下载队列')));
                                }
                              }, 
                              icon: const Icon(Icons.download_for_offline_rounded)
                            ),
                            const SizedBox(width: 8),
                            // 添加到列表
                            IconButton.filledTonal(
                              onPressed: () {
                                if (tracks.isNotEmpty) {
                                  final trackIds = tracks.map((t) => t.id).toList();
                                  showAddToPlaylistSheet(context, ref, trackIds); 
                                }
                              },
                              icon: const Icon(Icons.playlist_add_rounded),
                            ),
                            const SizedBox(width: 8),
                            // 随机播放
                            IconButton.outlined(
                              onPressed: () {
                                if (tracks.isNotEmpty) {
                                  final shuffled = List<Track>.from(tracks)..shuffle();
                                  ref.read(playerControllerProvider)?.playQueue(shuffled, 0);
                                }
                              }, 
                              icon: const Icon(Icons.shuffle_rounded),
                              style: IconButton.styleFrom(
                                side: BorderSide(color: Theme.of(context).colorScheme.outline.withOpacity(0.3)),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),

              if (widget.isPlaylist)
                SliverReorderableList(
                  itemCount: tracks.length,
                  itemBuilder: (context, index) {
                    final track = tracks[index];
                    // 核心加固：使用更独特的 Key 避免 Dismissible 冲突
                    return ReorderableDelayedDragStartListener(
                      key: ValueKey('reorder-tile-${track.id}'), 
                      index: index,
                      child: _TrackTile(
                        track: track, index: index, isPlaylist: true,
                        playlistId: tilePlaylistId,
                        downloadProgress: downloadStates.progress[track.id],
                        onDelete: () {
                          // 先通过本地 Provider 更新状态，再执行远程同步，防止 UI 闪烁导致 Key 失效
                          ref.read(playlistControllerProvider).removeTrack(widget.item.id, track.id);
                        },
                        onTap: () => ref.read(playerControllerProvider)?.playQueue(tracks, index),
                      ),
                    );
                  },
                  onReorder: (oldIdx, newIdx) => _handleReorder(tracks, oldIdx, newIdx),
                )
              else
                SliverList(
                  delegate: SliverChildBuilderDelegate(
                    (context, index) {
                      final track = tracks[index];
                      return _TrackTile(
                        track: track, index: index, 
                        playlistId: tilePlaylistId,
                        downloadProgress: downloadStates.progress[track.id],
                        onTap: () => ref.read(playerControllerProvider)?.playQueue(tracks, index),
                      );
                    },
                    childCount: tracks.length,
                  ),
                ),

              SliverPadding(
                padding: EdgeInsets.only(bottom: MediaQuery.of(context).padding.bottom + 100),
              ),
            ],
          );
        }
      ),
    );
  }
}

class _TrackTile extends ConsumerWidget {
  final Track track;
  final int index;
  final bool isPlaylist;
  final String? playlistId;
  final double? downloadProgress;
  final VoidCallback? onDelete;
  final VoidCallback onTap;

  const _TrackTile({
    required this.track, 
    required this.index, 
    this.isPlaylist = false, 
    this.playlistId,
    this.downloadProgress,
    this.onDelete,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    debugPrint('_TrackTile: Building for ${track.title}. isPlaylist=$isPlaylist, playlistId=$playlistId');
    final currentTrack = ref.watch(currentTrackProvider).value;
    final isPlaying = currentTrack?.id == track.id;
    final theme = Theme.of(context);

    Widget trailing;
    
    // 构造下载状态图标
    Widget? downloadStatus;
    if (downloadProgress != null) {
      downloadStatus = downloadProgress! < 1.0 
        ? SizedBox(width: 16, height: 16, child: CircularProgressIndicator(value: downloadProgress, strokeWidth: 2))
        : const Icon(Icons.check_circle_outline_rounded, size: 18, color: Colors.greenAccent);
    }

    // 组合 Trailing 区域
    trailing = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (downloadStatus != null) ...[
          downloadStatus,
          const SizedBox(width: 12),
        ],
        IconButton(
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(),
          icon: const Icon(Icons.more_vert_rounded),
          onPressed: () => showTrackActionSheet(context, ref, track, playlistId: playlistId),
        ),
        if (isPlaylist) ...[
          const SizedBox(width: 8),
          Icon(Icons.drag_indicator_rounded, color: theme.iconTheme.color?.withOpacity(0.3)),
        ],
      ],
    );

    Widget leading;
    if (isPlaying) {
      leading = SizedBox(width: 32, child: Center(child: PlayingVisualizer(color: theme.colorScheme.primary, size: 18)));
    } else {
      leading = SizedBox(width: 32, child: Text('${index + 1}', style: TextStyle(color: theme.textTheme.bodyMedium?.color?.withOpacity(0.5))));
    }

    Widget tile = ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 4),
      leading: leading,
      title: Text(
        track.title, 
        style: TextStyle(
          fontWeight: isPlaying ? FontWeight.bold : FontWeight.w600,
          color: isPlaying ? theme.colorScheme.primary : null,
        )
      ),
      subtitle: Text(track.format?.toUpperCase() ?? 'AUDIO', style: isPlaying ? TextStyle(color: theme.colorScheme.primary.withOpacity(0.7), fontSize: 10) : null),
      trailing: trailing,
      onTap: onTap,
    );

    if (isPlaylist) {
      return Dismissible(
        key: Key('dismiss-${track.id}'),
        direction: DismissDirection.endToStart,
        onDismissed: (_) => onDelete?.call(),
        background: Container(
          alignment: Alignment.centerRight,
          padding: const EdgeInsets.only(right: 20),
          color: Colors.redAccent,
          child: const Icon(Icons.remove_circle_outline_rounded, color: Colors.white),
        ),
        child: tile,
      );
    }

    return tile;
  }
}

class _PlaceholderCover extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      color: Theme.of(context).colorScheme.surfaceVariant,
      child: Icon(Icons.music_note_rounded, size: 100, color: Theme.of(context).iconTheme.color?.withOpacity(0.2)),
    );
  }
}
