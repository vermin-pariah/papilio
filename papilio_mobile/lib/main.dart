import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:audio_service/audio_service.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:permission_handler/permission_handler.dart';
import 'src/ui/home_view.dart';
import 'src/ui/player_view.dart';
import 'src/ui/playlist_view.dart';
import 'src/ui/settings_view.dart';
import 'src/ui/search_view.dart';
import 'src/ui/auth_view.dart';
import 'src/ui/server_config_view.dart';
import 'src/api/audio_handler.dart';
import 'src/api/music_repository.dart';
import 'src/providers/player_provider.dart';
import 'src/providers/theme_provider.dart';
import 'src/providers/settings_provider.dart';
import 'src/providers/auth_provider.dart';
import 'src/config/env_config.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'src/providers/navigation_provider.dart';
import 'src/ui/widgets/gradient_scaffold.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  
  final prefs = await SharedPreferences.getInstance();

  // Android 13+ 权限请求
  try {
    if (await Permission.notification.isDenied) {
      await Permission.notification.request();
    }
  } catch (e) {
    debugPrint('Permission request error: $e');
  }

  // Provide a small delay to let native plugins stabilize
  await Future.delayed(const Duration(milliseconds: 500));

  FlutterError.onError = (details) {
    debugPrint('Flutter Error: ${details.exception}');
  };

  try {
    final handler = await AudioService.init(
      builder: () => AudioPlayerHandler(),
      config: const AudioServiceConfig(
        androidNotificationChannelId: 'com.papilio.music.channel.audio',
        androidNotificationChannelName: 'Papilio Music Playback',
        androidNotificationOngoing: true,
      ),
    ).timeout(const Duration(seconds: 15));
    
    setGlobalAudioHandler(handler);
  } catch (e) {
    debugPrint('Failed to initialize AudioService: $e');
  }

  runApp(
    ProviderScope(
      overrides: [
        sharedPreferencesProvider.overrideWithValue(prefs),
      ],
      child: const PapilioApp(),
    ),
  );
}

class PapilioApp extends ConsumerWidget {
  const PapilioApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final accentColor = ref.watch(dynamicThemeProvider);

    return MaterialApp(
      title: 'Papilio',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        brightness: Brightness.dark,
        scaffoldBackgroundColor: const Color(0xFF0F172A),
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF0EA5E9), // 强制使用克莱因蓝/天空蓝作为种子色
          brightness: Brightness.dark,
          surface: const Color(0xFF1E293B),
          surfaceVariant: const Color(0xFF334155),
          primary: const Color(0xFF0EA5E9), // 更加深邃的克莱因蓝
          secondary: const Color(0xFF10B981), // 翡翠绿
          tertiary: const Color(0xFF6366F1), // 靛蓝色
        ),
        textTheme: GoogleFonts.interTextTheme(ThemeData.dark().textTheme).copyWith(
          displayLarge: GoogleFonts.montserrat(
            fontWeight: FontWeight.w900,
            letterSpacing: -1.5,
            color: Colors.white,
          ),
        ),
        useMaterial3: true,
      ),
      home: const AuthGate(),
    );
  }
}

class AuthGate extends ConsumerWidget {
  const AuthGate({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final envConfig = ref.watch(envConfigNotifierProvider);

    // 绝对优先级：只要未配置，立即去配置页，不看任何 Auth 状态
    if (!envConfig.isConfigured) {
      return const ServerConfigView();
    }

    final isLoggedIn = ref.watch(authStateProvider);

    if (isLoggedIn == null) {
      return GradientScaffold(
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const CircularProgressIndicator(),
              const SizedBox(height: 24),
              const Text('正在连接服务器...', style: TextStyle(color: Colors.white54)),
              const SizedBox(height: 48),
              TextButton.icon(
                onPressed: () => ref.read(envConfigNotifierProvider.notifier).resetConfig(),
                icon: const Icon(Icons.refresh_rounded, size: 16),
                label: const Text('重置服务器配置', style: TextStyle(color: Colors.white38)),
              ),
            ],
          ),
        ),
      );
    }

    return isLoggedIn ? const MainLayout() : const LoginView();
  }
}

class MainLayout extends ConsumerStatefulWidget {
  const MainLayout({super.key});

  @override
  ConsumerState<MainLayout> createState() => _MainLayoutState();
}

class _MainLayoutState extends ConsumerState<MainLayout> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final repo = ref.read(musicRepositoryProvider);
      ref.read(playerHandlerProvider)?.setRepository(repo);
      ref.read(playerControllerProvider)?.restoreLastState();
    });
  }

  @override
  Widget build(BuildContext context) {
    final currentIndex = ref.watch(navigationIndexProvider);

    // 强力全局监听：确保 Tab 切换时所有受影响的视图（如播放器）都能强制自检
    ref.listen(navigationIndexProvider, (prev, next) {
      debugPrint('Navigation switch: $prev -> $next');
      // 这里可以添加全局层面的状态同步逻辑
    });

    return GradientScaffold(
      body: IndexedStack(
        index: currentIndex,
        children: const [
          HomeView(),
          SearchView(),
          PlayerView(),
          PlaylistListView(),
          SettingsView(),
        ],
      ),
      bottomNavigationBar: currentIndex == 2 
        ? null 
        : NavigationBar(
            selectedIndex: currentIndex,
            onDestinationSelected: (index) => ref.read(navigationIndexProvider.notifier).state = index,
            backgroundColor: Colors.transparent,
            elevation: 0,
            destinations: const [
              NavigationDestination(icon: Icon(Icons.home_filled), label: '首页'),
              NavigationDestination(icon: Icon(Icons.search_rounded), label: '搜索'),
              NavigationDestination(icon: Icon(Icons.music_video_rounded), label: '音乐'),
              NavigationDestination(icon: Icon(Icons.library_music_rounded), label: '馆藏'),
              NavigationDestination(icon: Icon(Icons.person_rounded), label: '我的'),
            ],
          ),
    );
  }
}
