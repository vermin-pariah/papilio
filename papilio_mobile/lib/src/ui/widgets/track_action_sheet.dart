import 'dart:ui';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../../models/track.dart';
import '../../providers/player_provider.dart';
import '../../providers/playlist_provider.dart';
import '../../providers/download_provider.dart';
import '../../api/music_repository.dart';
import '../../config/env_config.dart';
import '../music_detail_view.dart';
import '../artist_detail_view.dart';
import '../playlist_view.dart';

void showTrackActionSheet(BuildContext context, WidgetRef ref, Track track, {String? playlistId}) {
  debugPrint('DEBUG: Opening TrackActionSheet for "${track.title}"');
  debugPrint('DEBUG: playlistId passed: $playlistId');
  debugPrint('DEBUG: isFavorite: ${track.isFavorite}');
  
  final config = ref.read(envConfigNotifierProvider);
  final theme = Theme.of(context);
  final coverUrl = track.albumId != null ? '${config.coversBaseUrl}${track.albumId}' : null;

  showModalBottomSheet(
    context: context,
    backgroundColor: Colors.transparent,
    barrierColor: Colors.black54,
    isScrollControlled: true,
    builder: (context) => Stack(
      children: [
        // 背景毛玻璃
        Positioned.fill(
          child: ClipRRect(
            borderRadius: const BorderRadius.vertical(top: Radius.circular(32)),
            child: BackdropFilter(
              filter: ImageFilter.blur(sigmaX: 20, sigmaY: 20),
              child: Container(color: const Color(0xFF0F172A).withOpacity(0.7)),
            ),
          ),
        ),
        
        SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // 顶部指示条
              const SizedBox(height: 12),
              Container(width: 40, height: 4, decoration: BoxDecoration(color: Colors.white24, borderRadius: BorderRadius.circular(2))),
              const SizedBox(height: 24),

              // 歌曲预览区
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 24),
                child: Row(
                  children: [
                    Container(
                      width: 64, height: 64,
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(12),
                        boxShadow: [BoxShadow(color: Colors.black26, blurRadius: 10, offset: const Offset(0, 4))],
                      ),
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(12),
                        child: coverUrl != null 
                          ? CachedNetworkImage(imageUrl: coverUrl, fit: BoxFit.cover)
                          : Container(color: Colors.white10, child: const Icon(Icons.music_note, color: Colors.white24)),
                      ),
                    ),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(track.title, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.bold, color: Colors.white)),
                          const SizedBox(height: 4),
                          Text(track.artistName ?? '未知艺术家', style: TextStyle(fontSize: 14, color: Colors.white.withOpacity(0.5))),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              
              const SizedBox(height: 24),
              const Divider(height: 1, color: Colors.white10),
              const SizedBox(height: 12),

              // 操作列表
              _ActionTile(
                icon: Icons.play_arrow_rounded, 
                title: '立即播放', 
                color: theme.colorScheme.primary,
                onTap: () {
                  Navigator.pop(context);
                  ref.read(playerControllerProvider)?.playTrack(track);
                },
              ),
              _ActionTile(
                icon: Icons.playlist_add_rounded, 
                title: '添加到播放列表', 
                onTap: () {
                  Navigator.pop(context);
                  showAddToPlaylistSheet(context, ref, track.id);
                },
              ),
              if (playlistId != null)
                _ActionTile(
                  icon: Icons.playlist_remove_rounded, 
                  title: '从当前播放列表移除', 
                  color: Colors.redAccent,
                  onTap: () async {
                    Navigator.pop(context);
                    try {
                      await ref.read(playlistControllerProvider).removeTrack(playlistId, track.id);
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text('已从列表移除: ${track.title}')),
                        );
                      }
                    } catch (e) {
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('移除失败: $e')));
                      }
                    }
                  },
                ),
              _ActionTile(
                icon: Icons.person_outline_rounded, 
                title: '查看艺术家', 
                onTap: () async {
                  Navigator.pop(context);
                  if (track.artistId != null) {
                    final repo = ref.read(musicRepositoryProvider);
                    final artist = await repo.getArtistById(track.artistId!);
                    if (context.mounted) Navigator.push(context, MaterialPageRoute(builder: (c) => ArtistDetailView(artist: artist)));
                  }
                },
              ),
              _ActionTile(
                icon: Icons.album_outlined, 
                title: '查看专辑', 
                onTap: () async {
                  Navigator.pop(context);
                  if (track.albumId != null) {
                    final repo = ref.read(musicRepositoryProvider);
                    final album = await repo.getAlbumById(track.albumId!);
                    if (context.mounted) Navigator.push(context, MaterialPageRoute(builder: (c) => MusicDetailView(item: album)));
                  }
                },
              ),
              StatefulBuilder(
                builder: (context, setSheetState) {
                  return _ActionTile(
                    icon: track.isFavorite ? Icons.favorite_rounded : Icons.favorite_border_rounded, 
                    title: track.isFavorite ? '取消收藏' : '收藏', 
                    color: track.isFavorite ? Colors.redAccent : null,
                    onTap: () async {
                      final wasFavorite = track.isFavorite;
                      setSheetState(() {
                        track.isFavorite = !wasFavorite;
                      });
                      
                      try {
                        await ref.read(musicRepositoryProvider).toggleFavorite(track.id);
                        ref.invalidate(currentTrackProvider);
                      } catch (e) {
                        setSheetState(() {
                          track.isFavorite = wasFavorite;
                        });
                      }
                    },
                  );
                },
              ),
              _ActionTile(
                icon: Icons.download_for_offline_rounded, 
                title: '下载到本地', 
                onTap: () {
                  Navigator.pop(context);
                  ref.read(downloadProvider.notifier).download(track);
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(
                      content: Text('正在下载: ${track.title}'),
                      backgroundColor: theme.colorScheme.primary,
                    )
                  );
                },
              ),
              const SizedBox(height: 32),
            ],
          ),
        ),
      ],
    ),
  );
}

class _ActionTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final Color? color;
  final VoidCallback onTap;

  const _ActionTile({required this.icon, required this.title, this.color, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 4),
      leading: Container(
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: (color ?? Colors.white).withOpacity(0.1),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Icon(icon, color: color ?? Colors.white70, size: 22),
      ),
      title: Text(title, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w500, color: Colors.white)),
      onTap: onTap,
    );
  }
}
