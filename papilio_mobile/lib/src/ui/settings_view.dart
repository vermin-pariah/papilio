import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image_picker/image_picker.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../config/env_config.dart';
import '../api/auth_repository.dart';
import '../api/music_repository.dart';
import '../providers/scan_provider.dart';
import '../providers/auth_provider.dart';
import '../providers/sleep_timer_provider.dart';
import '../models/user.dart';
import 'auth_view.dart';
import 'download_management_view.dart';

import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'admin_view.dart';

class SettingsView extends ConsumerStatefulWidget {
  const SettingsView({super.key});

  @override
  ConsumerState<SettingsView> createState() => _SettingsViewState();
}

class _SettingsViewState extends ConsumerState<SettingsView> {
  late TextEditingController _urlController;
  String _connectionStatus = "";
  bool _isTesting = false;
  Color _statusColor = Colors.white54;

  @override
  void initState() {
    super.initState();
    final currentUrl = ref.read(envConfigNotifierProvider).serverUrl;
    _urlController = TextEditingController(text: currentUrl);
  }

  @override
  void dispose() {
    _urlController.dispose();
    super.dispose();
  }

  Future<void> _testConnection() async {
    setState(() {
      _isTesting = true;
      _connectionStatus = "正在尝试连接...";
      _statusColor = Colors.white54;
    });

    try {
      final latency = await ref.read(musicRepositoryProvider).testConnection();
      if (!mounted) return;
      setState(() {
        _connectionStatus = "连接成功! 延迟: ${latency}ms";
        _statusColor = Colors.greenAccent;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _connectionStatus = "连接失败: ${e.toString().split(':').last}";
        _statusColor = Colors.redAccent;
      });
    } finally {
      if (mounted) setState(() => _isTesting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final scanStatus = ref.watch(scanStatusProvider).value;
    final isScanning = scanStatus?.isScanning ?? false;
    final envConfig = ref.watch(envConfigNotifierProvider);

    return Scaffold(
      backgroundColor: Colors.transparent,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        title: const Text('个人设置', style: TextStyle(fontWeight: FontWeight.bold)),
      ),
      body: ListView(
        padding: const EdgeInsets.all(20),
        children: [
          _ProfileSection(),
          
          const SizedBox(height: 32),

          _SectionHeader(title: '流媒体与数据'),
          Card(
            color: Theme.of(context).colorScheme.surface.withOpacity(0.4),
            elevation: 0,
            child: Column(
              children: [
                SwitchListTile(
                  secondary: const Icon(Icons.speed_rounded),
                  title: const Text('省流量模式', style: TextStyle(fontWeight: FontWeight.bold)),
                  subtitle: const Text('自动转码为 128kbps 以节省移动数据'),
                  value: envConfig.isDataSaverMode,
                  onChanged: (val) => ref.read(envConfigNotifierProvider.notifier).toggleDataSaver(val),
                ),
                const Divider(height: 1, indent: 64),
                Consumer(
                  builder: (context, ref, _) {
                    final sleepSeconds = ref.watch(sleepTimerProvider);
                    final timerNotifier = ref.read(sleepTimerProvider.notifier);
                    
                    return ListTile(
                      leading: const Icon(Icons.timer_outlined),
                      title: const Text('睡眠定时器', style: TextStyle(fontWeight: FontWeight.bold)),
                      subtitle: Text(sleepSeconds != null ? '剩余时间: ${timerNotifier.remainingLabel}' : '到达设定时间后停止播放'),
                      trailing: sleepSeconds != null 
                        ? IconButton(
                            icon: const Icon(Icons.close_rounded, size: 20),
                            onPressed: () => timerNotifier.cancelTimer(),
                          )
                        : const Icon(Icons.chevron_right_rounded),
                      onTap: () => _showSleepTimerDialog(context, ref),
                    );
                  },
                ),
                const Divider(height: 1, indent: 64),
                ListTile(
                  leading: const Icon(Icons.cleaning_services_rounded),
                  title: const Text('清除缓存'),
                  subtitle: const Text('删除临时文件和图片缓存 (不影响已下载歌曲)'),
                  onTap: () async {
                    final confirm = await showDialog<bool>(
                      context: context,
                      builder: (context) => AlertDialog(
                        title: const Text('确认清理'),
                        content: const Text('这将释放图片和流媒体产生的临时空间。您的“已下载”歌曲将安全保留。'),
                        actions: [
                          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('取消')),
                          TextButton(onPressed: () => Navigator.pop(context, true), child: const Text('清理')),
                        ],
                      ),
                    );

                    if (confirm == true) {
                      try {
                        // 清理临时目录 (just_audio 缓存等)
                        final tempDir = await getTemporaryDirectory();
                        if (await tempDir.exists()) {
                          await tempDir.list().forEach((f) => f.deleteSync(recursive: true));
                        }
                        
                        // 清理图片缓存
                        PaintingBinding.instance.imageCache.clear();
                        PaintingBinding.instance.imageCache.clearLiveImages();

                        if (mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('缓存已深度清理')));
                        }
                      } catch (e) {
                        if (mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('清理部分失败: $e')));
                        }
                      }
                    }
                  },
                ),
              ],
            ),
          ),

          const SizedBox(height: 24),

          _SectionHeader(title: '曲库管理'),
          Card(
            color: Theme.of(context).colorScheme.surface.withOpacity(0.4),
            elevation: 0,
            child: Column(
              children: [
                ListTile(
                  leading: const Icon(Icons.cloud_download_rounded),
                  title: const Text('下载管理', style: TextStyle(fontWeight: FontWeight.bold)),
                  subtitle: const Text('查看下载中和已下载的任务'),
                  trailing: const Icon(Icons.chevron_right_rounded),
                  onTap: () => Navigator.push(context, MaterialPageRoute(builder: (context) => const DownloadManagementView())),
                ),
                // 管理员专用入口
                Consumer(builder: (context, ref, _) {
                  final user = ref.watch(currentUserProvider).value;
                  if (user?.isAdmin != true) return const SizedBox.shrink();
                  return Column(
                    children: [
                      const Divider(height: 1, indent: 64),
                      ListTile(
                        leading: const Icon(Icons.admin_panel_settings_rounded, color: Colors.amber),
                        title: const Text('管理员控制台', style: TextStyle(fontWeight: FontWeight.bold)),
                        subtitle: const Text('系统状态监控与高级维护'),
                        trailing: const Icon(Icons.chevron_right_rounded),
                        onTap: () => Navigator.push(context, MaterialPageRoute(builder: (context) => const AdminConsoleView())),
                      ),
                    ],
                  );
                }),
              ],
            ),
          ),

          const SizedBox(height: 24),

          _SectionHeader(title: '服务器配置'),
          Card(
            color: Theme.of(context).colorScheme.surface.withOpacity(0.4),
            elevation: 0,
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('服务器地址 (API Host)', style: TextStyle(fontSize: 14, color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7))),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _urlController,
                    decoration: InputDecoration(
                      hintText: 'http://your-server-ip:3000',
                      border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
                      suffixIcon: IconButton(
                        icon: const Icon(Icons.save_rounded),
                        onPressed: () async {
                          await ref.read(envConfigNotifierProvider.notifier).updateServerUrl(_urlController.text);
                          if (mounted) {
                            ScaffoldMessenger.of(context).showSnackBar(
                              const SnackBar(content: Text('服务器地址已更新并持久化')),
                            );
                          }
                        },
                      ),
                    ),
                  ),
                  const SizedBox(height: 16),
                  Row(
                    children: [
                      OutlinedButton.icon(
                        onPressed: _isTesting ? null : _testConnection,
                        icon: _isTesting 
                          ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                          : const Icon(Icons.bolt_rounded, size: 18),
                        label: const Text('测试连接'),
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          _connectionStatus,
                          style: TextStyle(fontSize: 12, color: _statusColor, fontWeight: FontWeight.bold),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),

          const SizedBox(height: 32),

          _SectionHeader(title: '账号安全'),
          ListTile(
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
            leading: const Icon(Icons.lock_reset_rounded),
            title: const Text('修改密码'),
            onTap: () => _showChangePasswordDialog(context, ref),
          ),
          ListTile(
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
            tileColor: Theme.of(context).colorScheme.errorContainer.withOpacity(0.1),
            leading: Icon(Icons.logout_rounded, color: Theme.of(context).colorScheme.error),
            title: Text('退出登录', style: TextStyle(color: Theme.of(context).colorScheme.error, fontWeight: FontWeight.bold)),
            onTap: () => _showLogoutDialog(context, ref),
          ),

          const SizedBox(height: 48),

          const Center(
            child: Column(
              children: [
                Text('Papilio Music', style: TextStyle(fontWeight: FontWeight.bold, fontSize: 18)),
                Text('v1.0.0-beta • Flutter + Rust', style: TextStyle(color: Colors.white54)),
              ],
            ),
          ),
        ],
      ),
    );
  }

  void _showSleepTimerDialog(BuildContext context, WidgetRef ref) {
    final options = [10, 20, 30, 45, 60, 90];
    showModalBottomSheet(
      context: context,
      shape: const RoundedRectangleBorder(borderRadius: BorderRadius.vertical(top: Radius.circular(24))),
      builder: (context) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('开启睡眠定时器', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
            const SizedBox(height: 8),
            const Text('到达设定时间后，Papilio 将自动停止播放', style: TextStyle(color: Colors.white54, fontSize: 13)),
            const SizedBox(height: 20),
            Flexible(
              child: ListView.builder(
                shrinkWrap: true,
                itemCount: options.length,
                itemBuilder: (context, index) {
                  final mins = options[index];
                  return ListTile(
                    contentPadding: const EdgeInsets.symmetric(horizontal: 32),
                    title: Text('$mins 分钟'),
                    onTap: () {
                      ref.read(sleepTimerProvider.notifier).setTimer(mins);
                      Navigator.pop(context);
                    },
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _showChangePasswordDialog(BuildContext context, WidgetRef ref) {
    final controller = TextEditingController();
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('修改密码'),
        content: TextField(
          controller: controller,
          obscureText: true,
          autofocus: true,
          decoration: const InputDecoration(hintText: '输入新密码'),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          ElevatedButton(
            onPressed: () async {
              if (controller.text.isNotEmpty) {
                try {
                  await ref.read(authRepositoryProvider).updateProfile(password: controller.text);
                  Navigator.pop(context);
                  ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('密码修改成功')));
                } catch (e) {
                  ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('修改失败: $e')));
                }
              }
            },
            child: const Text('确认修改'),
          ),
        ],
      ),
    );
  }

  void _showLogoutDialog(BuildContext context, WidgetRef ref) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('退出登录'),
        content: const Text('确定要退出当前账号吗？'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          TextButton(
            onPressed: () async {
              await ref.read(authRepositoryProvider).logout();
              ref.read(authStateProvider.notifier).setLoggedIn(false);
              if (context.mounted) Navigator.pop(context);
            },
            child: const Text('退出', style: TextStyle(color: Colors.red)),
          ),
        ],
      ),
    );
  }
}

class _ProfileSection extends ConsumerWidget {
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final userAsync = ref.watch(currentUserProvider);
    final envConfig = ref.watch(envConfigNotifierProvider);

    return userAsync.when(
      loading: () => const Row(children: [CircularProgressIndicator()]),
      error: (e, _) => Text('加载失败: $e'),
      data: (user) {
        if (user == null) {
          return const Row(
            children: [
              CircleAvatar(radius: 35, child: Icon(Icons.person_off_rounded)),
              SizedBox(width: 20),
              Text("未登录", style: TextStyle(color: Colors.white38)),
            ],
          );
        }
        final serverUrl = envConfig.serverUrl ?? "";
        final avatarUrl = serverUrl.isNotEmpty ? user.getAvatarUrl(serverUrl) : null;
        
        return Row(
          children: [
            GestureDetector(
              onTap: () => _pickAndUploadAvatar(context, ref),
              child: Stack(
                children: [
                  CircleAvatar(
                    radius: 35,
                    backgroundColor: Theme.of(context).colorScheme.primaryContainer,
                    backgroundImage: avatarUrl != null ? CachedNetworkImageProvider(avatarUrl) : null,
                    child: avatarUrl == null 
                      ? Text(
                          user.displayName.isNotEmpty ? user.displayName.substring(0, 1).toUpperCase() : "?",
                          style: const TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
                        )
                      : null,
                  ),
                  Positioned(
                    bottom: 0,
                    right: 0,
                    child: Container(
                      padding: const EdgeInsets.all(4),
                      decoration: BoxDecoration(
                        color: Theme.of(context).colorScheme.primary,
                        shape: BoxShape.circle,
                      ),
                      child: const Icon(Icons.camera_alt_rounded, size: 14, color: Colors.white),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 20),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(user.displayName, style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold)),
                  Text("@${user.username}", style: const TextStyle(fontSize: 12, color: Colors.white38)),
                  Text(user.email ?? "未绑定邮箱", style: const TextStyle(fontSize: 14, color: Colors.white60)),
                ],
              ),
            ),
            IconButton(
              icon: const Icon(Icons.edit_note_rounded, color: Colors.white70),
              onPressed: () => _showEditProfileDialog(context, ref, user),
            ),
          ],
        );
      },
    );
  }

  Future<void> _pickAndUploadAvatar(BuildContext context, WidgetRef ref) async {
    final picker = ImagePicker();
    final image = await picker.pickImage(source: ImageSource.gallery, maxWidth: 512, maxHeight: 512, imageQuality: 85);
    
    if (image != null) {
      try {
        await ref.read(authRepositoryProvider).uploadAvatar(image.path);
        ref.invalidate(currentUserProvider);
        if (!context.mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('头像已成功更换')));
      } catch (e) {
        if (!context.mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('上传失败: $e')));
      }
    }
  }

  void _showEditProfileDialog(BuildContext context, WidgetRef ref, User user) {
    final nicknameController = TextEditingController(text: user.nickname);
    final emailController = TextEditingController(text: user.email);

    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('编辑个人资料'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: nicknameController,
              decoration: const InputDecoration(
                labelText: '昵称',
                hintText: '起个好听的名字吧',
                prefixIcon: Icon(Icons.face_rounded, size: 20),
              ),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: emailController,
              keyboardType: TextInputType.emailAddress,
              decoration: const InputDecoration(
                labelText: '邮箱',
                hintText: 'example@mail.com',
                prefixIcon: Icon(Icons.email_outlined, size: 20),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('取消')),
          ElevatedButton(
            onPressed: () async {
              try {
                await ref.read(authRepositoryProvider).updateProfile(
                  nickname: nicknameController.text.trim().isEmpty ? null : nicknameController.text.trim(),
                  email: emailController.text.trim().isEmpty ? null : emailController.text.trim(),
                );
                
                ref.invalidate(currentUserProvider);

                if (context.mounted) {
                  Navigator.pop(context);
                  ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('资料已更新')));
                }
              } catch (e) {
                if (context.mounted) {
                  ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('修改失败: $e')));
                }
              }
            },
            child: const Text('保存修改'),
          ),
        ],
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String title;
  const _SectionHeader({required this.title});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 4, bottom: 12),
      child: Text(
        title,
        style: TextStyle(
          fontSize: 14,
          fontWeight: FontWeight.bold,
          color: Theme.of(context).colorScheme.primary,
          letterSpacing: 1.2,
        ),
      ),
    );
  }
}