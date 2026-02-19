import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../api/music_repository.dart';
import '../config/env_config.dart';

final adminUserListProvider = FutureProvider.autoDispose<List<dynamic>>((ref) async {
  return await ref.watch(musicRepositoryProvider).listUsers();
});

class AdminUserListView extends ConsumerWidget {
  const AdminUserListView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final usersAsync = ref.watch(adminUserListProvider);
    final envConfig = ref.watch(envConfigNotifierProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('账号管理', style: TextStyle(fontWeight: FontWeight.bold)),
        backgroundColor: Colors.transparent,
        elevation: 0,
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh_rounded),
            onPressed: () => ref.invalidate(adminUserListProvider),
          ),
        ],
      ),
      body: usersAsync.when(
        data: (users) => ListView.separated(
          padding: const EdgeInsets.all(20),
          itemCount: users.length,
          separatorBuilder: (c, i) => const SizedBox(height: 12),
          itemBuilder: (context, index) {
            final user = users[index];
            final String userId = user['id'];
            final String username = user['username'];
            final String? nickname = user['nickname'];
            final String? email = user['email'];
            final bool isAdmin = user['is_admin'] ?? false;
            final String? avatar = user['avatar'];
            
            final serverUrl = envConfig.serverUrl ?? "";
            final avatarUrl = (avatar != null && serverUrl.isNotEmpty) 
                ? (serverUrl.endsWith('/') ? "${serverUrl}data/avatars/$avatar" : "$serverUrl/data/avatars/$avatar")
                : null;

            return Card(
              color: Colors.white.withOpacity(0.05),
              elevation: 0,
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
              child: ListTile(
                contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                leading: CircleAvatar(
                  backgroundColor: isAdmin ? Colors.amber.withOpacity(0.2) : Colors.blueGrey.withOpacity(0.2),
                  backgroundImage: avatarUrl != null ? CachedNetworkImageProvider(avatarUrl) : null,
                  child: avatarUrl == null ? Text(username[0].toUpperCase()) : null,
                ),
                title: Row(
                  children: [
                    Text(nickname ?? username, style: const TextStyle(fontWeight: FontWeight.bold)),
                    if (isAdmin)
                      Container(
                        margin: const EdgeInsets.only(left: 8),
                        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                        decoration: BoxDecoration(color: Colors.amber, borderRadius: BorderRadius.circular(4)),
                        child: const Text('ADMIN', style: TextStyle(fontSize: 8, color: Colors.black, fontWeight: FontWeight.bold)),
                      ),
                  ],
                ),
                subtitle: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('@$username', style: const TextStyle(fontSize: 12, color: Colors.white38)),
                    if (email != null) Text(email, style: const TextStyle(fontSize: 12, color: Colors.white38)),
                  ],
                ),
                trailing: PopupMenuButton<String>(
                  onSelected: (val) => _handleAction(context, ref, val, user),
                  itemBuilder: (c) => [
                    PopupMenuItem(
                      value: 'toggle_admin',
                      child: ListTile(
                        leading: Icon(isAdmin ? Icons.person_remove_rounded : Icons.admin_panel_settings_rounded, size: 20),
                        title: Text(isAdmin ? '取消管理员' : '设为管理员'),
                        contentPadding: EdgeInsets.zero,
                        visualDensity: VisualDensity.compact,
                      ),
                    ),
                    const PopupMenuItem(
                      value: 'delete',
                      child: ListTile(
                        leading: Icon(Icons.delete_forever_rounded, color: Colors.redAccent, size: 20),
                        title: Text('删除账号', style: TextStyle(color: Colors.redAccent)),
                        contentPadding: EdgeInsets.zero,
                        visualDensity: VisualDensity.compact,
                      ),
                    ),
                  ],
                ),
              ),
            );
          },
        ),
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('加载失败: $e')),
      ),
    );
  }

  void _handleAction(BuildContext context, WidgetRef ref, String action, dynamic user) async {
    final repo = ref.read(musicRepositoryProvider);
    final String userId = user['id'];
    final String username = user['username'];

    if (action == 'toggle_admin') {
      final bool currentIsAdmin = user['is_admin'] ?? false;
      try {
        await repo.updateUserRole(userId, !currentIsAdmin);
        ref.invalidate(adminUserListProvider);
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('已更新 $username 的权限')));
        }
      } catch (e) {
        if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('操作失败: $e')));
      }
    } else if (action == 'delete') {
      final confirm = await showDialog<bool>(
        context: context,
        builder: (c) => AlertDialog(
          title: const Text('危险操作'),
          content: Text('确定要彻底删除账号 "$username" 吗？此操作不可逆，将清除该用户的所有播放记录和收藏。'),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c, false), child: const Text('取消')),
            ElevatedButton(
              onPressed: () => Navigator.pop(c, true),
              style: ElevatedButton.styleFrom(backgroundColor: Colors.redAccent),
              child: const Text('确认删除'),
            ),
          ],
        ),
      );

      if (confirm == true) {
        try {
          await repo.deleteUser(userId);
          ref.invalidate(adminUserListProvider);
          if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('用户已删除')));
        } catch (e) {
          if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('删除失败: $e')));
        }
      }
    }
  }
}
