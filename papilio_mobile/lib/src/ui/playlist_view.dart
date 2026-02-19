import 'dart:ui';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../providers/playlist_provider.dart';
import '../config/env_config.dart';
import '../models/album.dart';
import 'music_detail_view.dart';
import 'download_management_view.dart';

class PlaylistListView extends ConsumerWidget {
  const PlaylistListView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final playlistsAsync = ref.watch(playlistsProvider);
    final favoritesAsync = ref.watch(favoritesPreviewProvider);
    final historyAsync = ref.watch(historyPreviewProvider);
    final config = ref.watch(envConfigNotifierProvider);

    return Scaffold(
      backgroundColor: Colors.transparent,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        title: const Text('我的馆藏', style: TextStyle(fontWeight: FontWeight.bold)),
        actions: [
          IconButton(
            icon: const Icon(Icons.add_rounded),
            onPressed: () => _showCreatePlaylistDialog(context, ref),
          ),
        ],
      ),
      body: RefreshIndicator(
        onRefresh: () async {
          ref.invalidate(playlistsProvider);
          ref.invalidate(favoritesPreviewProvider);
          ref.invalidate(historyPreviewProvider);
          await Future.wait([
            ref.read(playlistsProvider.future),
            ref.read(favoritesPreviewProvider.future),
            ref.read(historyPreviewProvider.future),
          ]);
        },
        child: CustomScrollView(
          physics: const AlwaysScrollableScrollPhysics(parent: BouncingScrollPhysics()),
          slivers: [
            playlistsAsync.when(
              data: (playlists) => SliverList(
                delegate: SliverChildBuilderDelegate(
                  (context, index) {
                    if (index == 0) {
                      // Favorites
                      final firstTrack = favoritesAsync.value?.firstOrNull;
                      final coverUrl = firstTrack?.albumId != null ? '${config.coversBaseUrl}${firstTrack!.albumId}' : null;
                      return _SpecialPlaylistTile(
                        title: '收藏',
                        subtitle: '所有标记红心的歌曲',
                        icon: Icons.favorite_rounded,
                        coverUrl: coverUrl,
                        gradient: const [Color(0xFF6366F1), Color(0xFFA855F7)], // Indigo to Purple
                        onTap: () => Navigator.push(context, MaterialPageRoute(
                          builder: (context) => MusicDetailView(
                            item: Album(id: 'favorites', title: '收藏', artistId: ''),
                            isFavorites: true,
                          ),
                        )),
                      );
                    }
                    if (index == 1) {
                      // Recently Played
                      final firstTrack = historyAsync.value?.firstOrNull;
                      final coverUrl = firstTrack?.albumId != null ? '${config.coversBaseUrl}${firstTrack!.albumId}' : null;
                      return _SpecialPlaylistTile(
                        title: '最近播放',
                        subtitle: '温故而知新',
                        icon: Icons.history_rounded,
                        coverUrl: coverUrl,
                        gradient: const [Color(0xFF2193b0), Color(0xFF6dd5ed)],
                        onTap: () => Navigator.push(context, MaterialPageRoute(
                          builder: (context) => MusicDetailView(
                            item: Album(id: 'history', title: '最近播放', artistId: ''),
                            isHistory: true,
                          ),
                        )),
                      );
                    }
                    if (index == 2) {
                      // Downloaded
                      return _SpecialPlaylistTile(
                        title: '已下载',
                        subtitle: '随时随地离线畅听',
                        icon: Icons.download_done_rounded,
                        gradient: const [Color(0xFF0EA5E9), Color(0xFF10B981)],
                        onTap: () => Navigator.push(context, MaterialPageRoute(
                          builder: (context) => const DownloadManagementView(),
                        )),
                      );
                    }

                    final playlistIndex = index - 3;
                    final playlist = playlists[playlistIndex];
                    return ListTile(
                      contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
                      leading: Container(
                        width: 56,
                        height: 56,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(12),
                          gradient: LinearGradient(
                            colors: [Theme.of(context).colorScheme.primary, Theme.of(context).colorScheme.tertiary],
                          ),
                        ),
                        child: const Icon(Icons.playlist_play_rounded, color: Colors.white),
                      ),
                      title: Text(playlist.title, style: const TextStyle(fontWeight: FontWeight.bold)),
                      subtitle: const Text('播放列表'),
                      trailing: SizedBox(
                        width: 48,
                        child: Center(
                          child: IconButton(
                            padding: EdgeInsets.zero,
                            icon: const Icon(Icons.more_vert_rounded),
                            onPressed: () => _showPlaylistOptions(context, ref, playlist),
                          ),
                        ),
                      ),
                      onTap: () {
                        Navigator.push(
                          context,
                          MaterialPageRoute(
                            builder: (context) => MusicDetailView(item: playlist, isPlaylist: true),
                          ),
                        );
                      },
                    );
                  },
                  childCount: playlists.length + 3,
                ),
              ),
              loading: () => const SliverFillRemaining(child: Center(child: CircularProgressIndicator())),
              error: (err, _) => SliverFillRemaining(child: Center(child: Text('加载失败: $err'))),
            ),
            SliverPadding(
              padding: EdgeInsets.only(bottom: MediaQuery.of(context).padding.bottom + 100),
            ),
          ],
        ),
      ),
    );
  }

  void _showPlaylistOptions(BuildContext context, WidgetRef ref, Album playlist) {
    showModalBottomSheet(
      context: context,
      backgroundColor: Colors.transparent,
      isScrollControlled: true,
      builder: (context) => Stack(
        children: [
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
                const SizedBox(height: 12),
                Container(width: 40, height: 4, decoration: BoxDecoration(color: Colors.white24, borderRadius: BorderRadius.circular(2))),
                const SizedBox(height: 24),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 24),
                  child: Row(
                    children: [
                      Container(
                        width: 64, height: 64,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(12),
                          gradient: LinearGradient(colors: [Theme.of(context).colorScheme.primary, Theme.of(context).colorScheme.tertiary]),
                        ),
                        child: const Icon(Icons.playlist_play_rounded, color: Colors.white, size: 32),
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(playlist.title, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.bold, color: Colors.white)),
                            const Text('播放列表', style: TextStyle(fontSize: 14, color: Colors.white54)),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 24),
                const Divider(height: 1, color: Colors.white10),
                const SizedBox(height: 12),
                _PlaylistActionTile(
                  icon: Icons.edit_rounded, 
                  title: '重命名', 
                  onTap: () {
                    Navigator.pop(context);
                    _showEditPlaylistDialog(context, ref, playlist);
                  },
                ),
                _PlaylistActionTile(
                  icon: Icons.delete_outline_rounded, 
                  title: '删除列表', 
                  color: Colors.redAccent,
                  onTap: () async {
                    Navigator.pop(context);
                    final confirm = await _showDeleteConfirm(context);
                    if (confirm == true) {
                      ref.read(playlistControllerProvider).delete(playlist.id);
                    }
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

  void _showEditPlaylistDialog(BuildContext context, WidgetRef ref, Album playlist) {
    final controller = TextEditingController(text: playlist.title);
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('重命名播放列表'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(hintText: '列表名称'),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          ElevatedButton(
            onPressed: () {
              if (controller.text.isNotEmpty) {
                ref.read(playlistControllerProvider).update(playlist.id, controller.text);
                Navigator.pop(context);
              }
            },
            child: const Text('保存'),
          ),
        ],
      ),
    );
  }

  Future<bool?> _showDeleteConfirm(BuildContext context) {
    return showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('确认删除'),
        content: const Text('确定要永久删除这个播放列表吗？此操作不可撤销。'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('取消')),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除', style: TextStyle(color: Colors.red)),
          ),
        ],
      ),
    );
  }

  void _showCreatePlaylistDialog(BuildContext context, WidgetRef ref) {
    final controller = TextEditingController();
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('新建播放列表'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(hintText: '列表名称'),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          ElevatedButton(
            onPressed: () {
              if (controller.text.isNotEmpty) {
                ref.read(playlistControllerProvider).create(controller.text);
                Navigator.pop(context);
              }
            },
            child: const Text('创建'),
          ),
        ],
      ),
    );
  }
}

class _PlaylistActionTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final Color? color;
  final VoidCallback onTap;

  const _PlaylistActionTile({required this.icon, required this.title, this.color, required this.onTap});

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

class _EmptyPlaylists extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.playlist_add_rounded, size: 80, color: Colors.white.withOpacity(0.1)),
          const SizedBox(height: 16),
          Text('还没有播放列表', style: TextStyle(color: Colors.white.withOpacity(0.5))),
        ],
      ),
    );
  }
}

class _SpecialPlaylistTile extends StatelessWidget {
  final String title;
  final String subtitle;
  final IconData icon;
  final String? coverUrl;
  final List<Color> gradient;
  final VoidCallback onTap;

  const _SpecialPlaylistTile({
    required this.title,
    required this.subtitle,
    required this.icon,
    this.coverUrl,
    required this.gradient,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
      leading: Container(
        width: 56,
        height: 56,
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(16),
          gradient: coverUrl == null ? LinearGradient(
            colors: gradient,
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
          ) : null,
          boxShadow: [
            BoxShadow(
              color: (coverUrl == null ? gradient.first : Colors.black).withOpacity(0.3),
              blurRadius: 8,
              offset: const Offset(0, 4),
            ),
          ],
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(16),
          child: coverUrl != null 
            ? CachedNetworkImage(
                imageUrl: coverUrl!,
                fit: BoxFit.cover,
                placeholder: (context, url) => Container(
                  decoration: BoxDecoration(gradient: LinearGradient(colors: gradient)),
                  child: Icon(icon, color: Colors.white, size: 28),
                ),
                errorWidget: (context, url, error) => Container(
                  decoration: BoxDecoration(gradient: LinearGradient(colors: gradient)),
                  child: Icon(icon, color: Colors.white, size: 28),
                ),
              )
            : Icon(icon, color: Colors.white, size: 28),
        ),
      ),
      title: Text(title, style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
      subtitle: Text(subtitle, style: TextStyle(fontSize: 12, color: Colors.white.withOpacity(0.5))),
      onTap: onTap,
    );
  }
}

void showAddToPlaylistSheet(BuildContext context, WidgetRef ref, dynamic trackIdOrIds) {
  final List<String> trackIds = trackIdOrIds is String ? [trackIdOrIds] : List<String>.from(trackIdOrIds);

  showModalBottomSheet(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    builder: (context) => Stack(
      children: [
        Positioned.fill(
          child: ClipRRect(
            borderRadius: const BorderRadius.vertical(top: Radius.circular(32)),
            child: BackdropFilter(
              filter: ImageFilter.blur(sigmaX: 20, sigmaY: 20),
              child: Container(color: const Color(0xFF0F172A).withOpacity(0.8)),
            ),
          ),
        ),
        SafeArea(
          child: Material( // 添加 Material 祖先
            color: Colors.transparent,
            child: Padding(
              padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const SizedBox(height: 12),
                  Container(width: 40, height: 4, decoration: BoxDecoration(color: Colors.white24, borderRadius: BorderRadius.circular(2))),
                  const Padding(
                    padding: EdgeInsets.all(24),
                    child: Text('添加到播放列表', style: TextStyle(fontSize: 20, fontWeight: FontWeight.bold, color: Colors.white)),
                  ),
                  
                  // 新建列表按钮
                  ListTile(
                    contentPadding: const EdgeInsets.symmetric(horizontal: 24),
                    leading: Container(
                      padding: const EdgeInsets.all(8),
                      decoration: BoxDecoration(color: Theme.of(context).colorScheme.primary.withOpacity(0.1), borderRadius: BorderRadius.circular(10)),
                      child: Icon(Icons.add_rounded, color: Theme.of(context).colorScheme.primary),
                    ),
                    title: const Text('新建播放列表', style: TextStyle(color: Colors.white, fontWeight: FontWeight.w500)),
                    onTap: () {
                      Navigator.pop(context);
                      _showCreatePlaylistDialogFromSheet(context, ref, trackIds);
                    },
                  ),
                  const Divider(height: 24, color: Colors.white10, indent: 24, endIndent: 24),

                  Consumer(builder: (context, ref, _) {
                    final playlistsAsync = ref.watch(playlistsProvider);
                    return playlistsAsync.when(
                      data: (playlists) {
                        if (playlists.isEmpty) {
                          return const Padding(
                            padding: EdgeInsets.symmetric(vertical: 40),
                            child: Text('暂无播放列表', style: TextStyle(color: Colors.white38)),
                          );
                        }
                        return ConstrainedBox(
                          constraints: BoxConstraints(maxHeight: MediaQuery.of(context).size.height * 0.4),
                          child: ListView.builder(
                            shrinkWrap: true,
                            itemCount: playlists.length,
                            itemBuilder: (context, index) {
                              final p = playlists[index];
                              return ListTile(
                                contentPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 4),
                                leading: Container(
                                  width: 40, height: 40,
                                  decoration: BoxDecoration(
                                    borderRadius: BorderRadius.circular(8),
                                    gradient: LinearGradient(colors: [Theme.of(context).colorScheme.primary.withOpacity(0.5), Theme.of(context).colorScheme.tertiary.withOpacity(0.5)]),
                                  ),
                                  child: const Icon(Icons.playlist_play_rounded, color: Colors.white, size: 20),
                                ),
                                title: Text(p.title, style: const TextStyle(color: Colors.white)),
                                onTap: () async {
                                  try {
                                    if (trackIds.length == 1) {
                                      await ref.read(playlistControllerProvider).addTrack(p.id, trackIds.first);
                                    } else {
                                      await ref.read(playlistControllerProvider).addTracks(p.id, trackIds);
                                    }
                                    
                                    if (context.mounted) {
                                      Navigator.pop(context);
                                      ScaffoldMessenger.of(context).showSnackBar(
                                        SnackBar(content: Text('已添加到 ${p.title}'), backgroundColor: Theme.of(context).colorScheme.primary),
                                      );
                                    }
                                  } catch (e) {
                                    if (context.mounted) {
                                      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('添加失败: $e')));
                                    }
                                  }
                                },
                              );
                            },
                          ),
                        );
                      },
                      loading: () => const Padding(padding: EdgeInsets.all(40), child: CircularProgressIndicator()),
                      error: (e, _) => Padding(padding: const EdgeInsets.all(40), child: Text('加载失败: $e', style: const TextStyle(color: Colors.redAccent))),
                    );
                  }),
                  const SizedBox(height: 32),
                ],
              ),
            ),
          ),
        ),
      ],
    ),
  );
}

void _showCreatePlaylistDialogFromSheet(BuildContext context, WidgetRef ref, List<String> trackIds) {
  final controller = TextEditingController();
  showDialog(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('新建播放列表'),
      content: TextField(
        controller: controller,
        autofocus: true,
        decoration: const InputDecoration(hintText: '列表名称'),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
        ElevatedButton(
          onPressed: () async {
            if (controller.text.isNotEmpty) {
              try {
                await ref.read(playlistControllerProvider).create(controller.text);
                // 重新弹出添加列表，此时应该已经包含新列表
                if (context.mounted) {
                  Navigator.pop(context);
                  showAddToPlaylistSheet(context, ref, trackIds);
                }
              } catch (e) {
                if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('创建失败: $e')));
              }
            }
          },
          child: const Text('创建'),
        ),
      ],
    ),
  );
}