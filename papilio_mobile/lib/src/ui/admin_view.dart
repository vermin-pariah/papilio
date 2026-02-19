import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../providers/scan_provider.dart';
import '../config/env_config.dart';
import '../api/music_repository.dart';
import '../models/artist_sync_status.dart';

import 'admin_user_view.dart';

// 管理员配置 Provider
final adminConfigProvider = FutureProvider<Map<String, dynamic>>((ref) async {
  return await ref.watch(musicRepositoryProvider).getAdminConfig();
});

// 系统状态 Provider
final systemStatusProvider = StreamProvider<Map<String, dynamic>>((ref) {
  final repo = ref.watch(musicRepositoryProvider);
  return Stream.periodic(const Duration(seconds: 3)).asyncMap((_) => repo.getSystemStatus());
});

final artistSyncStatusProvider = StreamProvider<ArtistSyncStatus>((ref) {
  final repo = ref.watch(musicRepositoryProvider);
  return Stream.periodic(const Duration(seconds: 2)).asyncMap((_) => repo.getArtistSyncStatus());
});

class AdminConsoleView extends ConsumerWidget {
  const AdminConsoleView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scanStatus = ref.watch(scanStatusProvider).value;
    final isScanning = scanStatus?.isScanning ?? false;
    final adminConfigAsync = ref.watch(adminConfigProvider);
    final systemStatusAsync = ref.watch(systemStatusProvider);
    final artistSyncStatus = ref.watch(artistSyncStatusProvider).value;
    final isArtistSyncing = artistSyncStatus?.isSyncing ?? false;

    return Scaffold(
      appBar: AppBar(
        title: const Text('系统管理', style: TextStyle(fontWeight: FontWeight.bold)),
        backgroundColor: Colors.transparent,
        elevation: 0,
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh_rounded),
            onPressed: () {
              ref.invalidate(adminConfigProvider);
              ref.invalidate(systemStatusProvider);
            },
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(20),
        children: [
          _buildSectionHeader(context, '系统健康度'),
          Card(
            color: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.3),
            child: systemStatusAsync.when(
              data: (status) => Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceAround,
                  children: [
                    _buildStatusItem('系统状态', '在线', Colors.greenAccent),
                  ],
                ),
              ),
              loading: () => const Padding(padding: EdgeInsets.all(16), child: Center(child: CircularProgressIndicator())),
              error: (e, _) => ListTile(title: const Text('无法获取状态'), subtitle: Text(e.toString())),
            ),
          ),

          const SizedBox(height: 24),
          _buildSectionHeader(context, '曲库自动化'),
          Card(
            color: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.3),
            child: Column(
              children: [
                ListTile(
                  leading: Icon(
                    isScanning ? Icons.sync_rounded : Icons.cloud_sync_rounded, 
                    color: isScanning ? Theme.of(context).colorScheme.primary : null
                  ),
                  title: const Text('同步曲库', style: TextStyle(fontWeight: FontWeight.bold)),
                  subtitle: Text(isScanning ? '正在扫描: ${scanStatus?.currentCount ?? 0} / ${scanStatus?.totalCount ?? 0}' : '手动触发服务器扫描音乐目录'),
                  trailing: isScanning 
                    ? Text('${((scanStatus?.progress ?? 0) * 100).toInt()}%', style: TextStyle(color: Theme.of(context).colorScheme.primary, fontWeight: FontWeight.bold))
                    : const Icon(Icons.play_arrow_rounded),
                  onTap: () async {
                    if (isScanning) {
                      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('扫描正在进行中，请稍候...')));
                      return;
                    }
                    final success = await ref.read(scanControllerProvider).startScan();
                    if (context.mounted) {
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(content: Text(success ? '已向服务器发起同步请求' : '同步请求发送失败')),
                      );
                    }
                  },
                ),
                if (isScanning)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(64, 0, 16, 16),
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(4),
                      child: LinearProgressIndicator(
                        value: scanStatus?.progress ?? 0,
                        minHeight: 6,
                        backgroundColor: Colors.white10,
                  ),
                    ),
                  ),
                const Divider(height: 1, indent: 64),
                ListTile(
                  leading: Icon(
                    isArtistSyncing ? Icons.sync_rounded : Icons.person_search_rounded,
                    color: isArtistSyncing ? Colors.cyanAccent : null
                  ),
                  title: const Text('同步歌手信息', style: TextStyle(fontWeight: FontWeight.bold)),
                  subtitle: Text(isArtistSyncing 
                    ? '进度: ${artistSyncStatus?.currentCount ?? 0} / ${artistSyncStatus?.totalCount ?? 0}' 
                    : '补全缺失的歌手 MusicBrainz 元数据'),
                  trailing: isArtistSyncing 
                    ? Text('${((artistSyncStatus?.progress ?? 0) * 100).toInt()}%', style: const TextStyle(color: Colors.cyanAccent, fontWeight: FontWeight.bold))
                    : const Icon(Icons.play_arrow_rounded),
                  onTap: () async {
                    if (isArtistSyncing) {
                      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('歌手同步正在进行中...')));
                      return;
                    }
                    try {
                      await ref.read(musicRepositoryProvider).triggerArtistSync();
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('歌手同步任务已启动')));
                      }
                    } catch (e) {
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('启动失败: $e')));
                      }
                    }
                  },
                ),
                if (isArtistSyncing)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(64, 0, 16, 16),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        ClipRRect(
                          borderRadius: BorderRadius.circular(4),
                          child: LinearProgressIndicator(
                            value: artistSyncStatus?.progress ?? 0,
                            minHeight: 6,
                            backgroundColor: Colors.white10,
                            color: Colors.cyanAccent,
                          ),
                        ),
                        if (artistSyncStatus?.lastError != null)
                          Padding(
                            padding: const EdgeInsets.only(top: 8),
                            child: Text(
                              '错误: ${artistSyncStatus?.lastError}',
                              style: const TextStyle(color: Colors.redAccent, fontSize: 10),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                      ],
                    ),
                  ),
                const Divider(height: 1, indent: 64),
                ListTile(
                  leading: const Icon(Icons.no_photography_rounded, color: Colors.cyanAccent),
                  title: const Text('补全缺失歌手头像'),
                  subtitle: const Text('仅针对目前没有头像的歌手尝试抓取'),
                  onTap: () async {
                    if (isArtistSyncing) {
                      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('歌手同步正在进行中...')));
                      return;
                    }
                    try {
                      await ref.read(musicRepositoryProvider).triggerArtistSyncMissing();
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('缺失歌手同步任务已启动')));
                      }
                    } catch (e) {
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('启动失败: $e')));
                      }
                    }
                  },
                ),
                const Divider(height: 1, indent: 64),
                ListTile(
                  leading: const Icon(Icons.folder_copy_rounded, color: Colors.amberAccent),
                  title: const Text('整理曲库文件', style: TextStyle(fontWeight: FontWeight.bold)),
                  subtitle: const Text('按 歌手/专辑 自动归类物理文件并同步资产'),
                  onTap: () async {
                    if (isScanning || isArtistSyncing) {
                      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('有其他同步任务正在进行，请稍后触发整理')));
                      return;
                    }
                    try {
                      await ref.read(musicRepositoryProvider).triggerLibraryOrganize();
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('曲库整理任务已在后台启动')));
                      }
                    } catch (e) {
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('启动失败: $e')));
                      }
                    }
                  },
                ),
              ],
            ),
          ),
          
          const SizedBox(height: 24),
          
          _buildSectionHeader(context, '用户管理'),
          Card(
            color: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.3),
            child: ListTile(
              leading: const Icon(Icons.people_alt_rounded),
              title: const Text('账号列表'),
              subtitle: const Text('查看并管理所有注册用户'),
              trailing: const Icon(Icons.chevron_right_rounded),
              onTap: () {
                Navigator.push(context, MaterialPageRoute(builder: (c) => const AdminUserListView()));
              },
            ),
          ),

          const SizedBox(height: 48),
          const Center(
            child: Text('PAPILIO ADMIN PROTOCOL v1.0', style: TextStyle(color: Colors.white10, fontSize: 10, letterSpacing: 2)),
          ),
        ],
      ),
    );
  }

  Widget _buildSectionHeader(BuildContext context, String title) {
    return Padding(
      padding: const EdgeInsets.only(left: 4, bottom: 8),
      child: Text(title, style: TextStyle(fontSize: 13, color: Theme.of(context).colorScheme.primary, fontWeight: FontWeight.bold)),
    );
  }

  Widget _buildStatusItem(String label, String value, Color color) {
    return Column(
      children: [
        Text(value, style: TextStyle(fontSize: 20, fontWeight: FontWeight.bold, color: color)),
        const SizedBox(height: 4),
        Text(label, style: const TextStyle(fontSize: 10, color: Colors.white38)),
      ],
    );
  }
}
