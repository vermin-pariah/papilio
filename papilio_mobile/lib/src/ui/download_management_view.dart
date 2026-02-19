import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../providers/download_provider.dart';
import '../providers/player_provider.dart';
import '../config/env_config.dart';
import '../models/track.dart';
import 'widgets/gradient_scaffold.dart';
import 'widgets/track_action_sheet.dart';
import 'widgets/playing_visualizer.dart';

class DownloadManagementView extends ConsumerWidget {
  const DownloadManagementView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return DefaultTabController(
      length: 2,
      child: GradientScaffold(
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          title: const Text('下载管理', style: TextStyle(fontWeight: FontWeight.bold)),
          bottom: TabBar(
            tabs: const [
              Tab(text: '下载中'),
              Tab(text: '已下载'),
            ],
            indicatorColor: Theme.of(context).colorScheme.primary,
            labelColor: Theme.of(context).colorScheme.primary,
            unselectedLabelColor: Colors.white54,
          ),
        ),
        body: const TabBarView(
          children: [
            _DownloadingList(),
            _CompletedList(),
          ],
        ),
      ),
    );
  }
}

class _DownloadingList extends ConsumerWidget {
  const _DownloadingList();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(downloadProvider);
    final downloading = state.downloading;

    if (downloading.isEmpty) {
      return _EmptyState(icon: Icons.cloud_download_outlined, message: '暂无正在下载的任务');
    }

    return ListView.builder(
      padding: const EdgeInsets.symmetric(vertical: 12),
      itemCount: downloading.length,
      itemBuilder: (context, index) {
        final track = downloading[index];
        final progress = state.progress[track.id] ?? 0.0;

        return ListTile(
          contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
          leading: _TrackCover(track: track),
          title: Text(track.title, style: const TextStyle(fontWeight: FontWeight.bold)),
          subtitle: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(track.artistName ?? '未知艺术家', style: const TextStyle(fontSize: 12, color: Colors.white54)),
              const SizedBox(height: 8),
              LinearProgressIndicator(
                value: progress,
                backgroundColor: Colors.white10,
                borderRadius: BorderRadius.circular(4),
              ),
            ],
          ),
          trailing: Text('${(progress * 100).toInt()}%', style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: Color(0xFF0EA5E9))),
        );
      },
    );
  }
}

class _CompletedList extends ConsumerWidget {
  const _CompletedList();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(downloadProvider);
    final completed = state.completed;

    if (completed.isEmpty) {
      return _EmptyState(icon: Icons.download_done_rounded, message: '还没有已下载的歌曲');
    }

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 16, 12, 8),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text('共 ${completed.length} 首歌曲', style: const TextStyle(color: Colors.white54, fontSize: 13)),
              TextButton.icon(
                onPressed: () => _confirmDeleteAll(context, ref),
                icon: const Icon(Icons.delete_sweep_rounded, size: 18, color: Colors.redAccent),
                label: const Text('全部删除', style: TextStyle(color: Colors.redAccent, fontSize: 13)),
              ),
            ],
          ),
        ),
        const Divider(height: 1, color: Colors.white10),
        Expanded(
          child: ListView.builder(
            padding: const EdgeInsets.symmetric(vertical: 4),
            itemCount: completed.length,
            itemBuilder: (context, index) {
              final track = completed[index];
              final currentTrack = ref.watch(currentTrackProvider).value;
              final isPlaying = currentTrack?.id == track.id;

              return ListTile(
                contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
                leading: Stack(
                  children: [
                    _TrackCover(track: track),
                    if (isPlaying)
                      Container(
                        width: 50, height: 50,
                        decoration: BoxDecoration(borderRadius: BorderRadius.circular(8), color: Colors.black38),
                                                  child: const Center(child: PlayingVisualizer(size: 18)),
                        
                      ),
                  ],
                ),
                title: Text(
                  track.title, 
                  style: TextStyle(
                    fontWeight: isPlaying ? FontWeight.bold : FontWeight.w600,
                    color: isPlaying ? const Color(0xFF0EA5E9) : null,
                  )
                ),
                subtitle: FutureBuilder<double>(
                  future: ref.read(downloadProvider.notifier).getFileSize(track.id),
                  builder: (context, snapshot) {
                    final size = snapshot.data ?? 0.0;
                    return Text(
                      '${track.artistName ?? '未知艺术家'} • ${size > 0 ? size.toStringAsFixed(1) : '...'} MB', 
                      style: const TextStyle(fontSize: 12)
                    );
                  },
                ),
                trailing: IconButton(
                  icon: const Icon(Icons.delete_outline_rounded, color: Colors.white24, size: 20),
                  onPressed: () => _confirmDelete(context, ref, track),
                ),
                onTap: () => ref.read(playerControllerProvider)?.playTrack(track),
                onLongPress: () => showTrackActionSheet(context, ref, track),
              );
            },
          ),
        ),
      ],
    );
  }

  void _confirmDeleteAll(BuildContext context, WidgetRef ref) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('清空下载'),
        content: const Text('确定要删除所有已下载的本地歌曲吗？此操作不可撤销。'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          TextButton(
            onPressed: () {
              ref.read(downloadProvider.notifier).clearAllDownloads();
              Navigator.pop(context);
            },
            child: const Text('全部删除', style: TextStyle(color: Colors.redAccent)),
          ),
        ],
      ),
    );
  }

  void _confirmDelete(BuildContext context, WidgetRef ref, Track track) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('删除下载'),
        content: Text('确定要删除歌曲 "${track.title}" 的本地文件吗？'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          TextButton(
            onPressed: () {
              ref.read(downloadProvider.notifier).deleteDownload(track.id);
              Navigator.pop(context);
            },
            child: const Text('删除', style: TextStyle(color: Colors.redAccent)),
          ),
        ],
      ),
    );
  }
}

class _TrackCover extends ConsumerWidget {
  final Track track;
  const _TrackCover({required this.track});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final config = ref.watch(envConfigNotifierProvider);
    final coverUrl = track.albumId != null ? '${config.coversBaseUrl}${track.albumId}' : null;

    return Container(
      width: 50, height: 50,
      decoration: BoxDecoration(borderRadius: BorderRadius.circular(8), color: Colors.white10),
      child: coverUrl != null 
        ? ClipRRect(borderRadius: BorderRadius.circular(8), child: CachedNetworkImage(imageUrl: coverUrl, fit: BoxFit.cover))
        : const Icon(Icons.music_note_rounded, color: Colors.white24),
    );
  }
}

class _EmptyState extends StatelessWidget {
  final IconData icon;
  final String message;
  const _EmptyState({required this.icon, required this.message});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(icon, size: 64, color: Colors.white10),
          const SizedBox(height: 16),
          Text(message, style: const TextStyle(color: Colors.white38)),
        ],
      ),
    );
  }
}
