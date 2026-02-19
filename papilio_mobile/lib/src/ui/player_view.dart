import 'dart:ui';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:audio_service/audio_service.dart';
import 'package:audio_video_progress_bar/audio_video_progress_bar.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:scrollable_positioned_list/scrollable_positioned_list.dart';
import '../models/track.dart';
import '../models/lyric.dart';
import '../models/scan_status.dart';
import '../providers/player_provider.dart';
import '../providers/lyrics_provider.dart';
import '../providers/download_provider.dart';
import '../api/music_repository.dart';
import '../config/env_config.dart';
import 'music_detail_view.dart';
import 'artist_detail_view.dart';
// import 'widgets/lyric_calibration_sheet.dart'; // Removed
import 'widgets/playing_visualizer.dart';
import '../providers/navigation_provider.dart';

class PlayerView extends ConsumerStatefulWidget {
  const PlayerView({super.key});

  @override
  ConsumerState<PlayerView> createState() => _PlayerViewState();
}

class _PlayerViewState extends ConsumerState<PlayerView> with SingleTickerProviderStateMixin {
  late PageController _pageController;
  late AnimationController _vinylController;
  int _currentPage = 1; // 保持一致
  bool _isUserScrolling = false;
  Timer? _scrollTimer;
  
  // Optimistic UI state for mode switching
  AudioServiceRepeatMode? _optimisticRepeatMode;
  AudioServiceShuffleMode? _optimisticShuffleMode;

  @override
  void initState() {
    super.initState();
    _pageController = PageController(initialPage: 1);
    _vinylController = AnimationController(vsync: this, duration: const Duration(seconds: 20));
  }

  @override
  void dispose() {
    _pageController.dispose();
    _vinylController.dispose();
    _scrollTimer?.cancel();
    super.dispose();
  }

  void _handleUserScroll() {
    setState(() => _isUserScrolling = true);
    _scrollTimer?.cancel();
    _scrollTimer = Timer(const Duration(seconds: 5), () {
      if (mounted) setState(() => _isUserScrolling = false);
    });
  }

  @override
  Widget build(BuildContext context) {
    final currentTrackAsync = ref.watch(currentTrackProvider);
    
    // 监听导航索引：点击 Tab 时重置
    ref.listen(navigationIndexProvider, (previous, next) {
      if (next == 2 && _pageController.hasClients) {
        _pageController.jumpToPage(1);
        if (mounted) setState(() => _currentPage = 1);
      }
    });

    // 监听当前歌曲变化：切歌时如果正在播放器界面，强制校正到唱片页
    ref.listen(currentTrackProvider, (prev, next) {
      final navIndex = ref.read(navigationIndexProvider);
      
      // 核心修复：只有当 ID 真正改变（且不为 null）时才重置页面
      final prevId = prev?.value?.id;
      final nextId = next.value?.id;
      
      if (navIndex == 2 && nextId != null && nextId != prevId && _pageController.hasClients) {
        _pageController.animateToPage(1, duration: const Duration(milliseconds: 300), curve: Curves.easeOut);
        if (mounted) setState(() => _currentPage = 1);
      }
    });

    return currentTrackAsync.when(
      data: (mediaItem) {
        if (mediaItem == null) return _buildEmptyState();

        final artistName = mediaItem.extras?['artistName'] as String? ?? mediaItem.artist ?? 'Unknown Artist';
        final albumTitle = mediaItem.extras?['albumTitle'] as String? ?? mediaItem.album ?? 'Unknown Album';

        // Partial listener for vinyl animation
        ref.listen(playbackStateProvider, (previous, next) {
          final isPlaying = next.value?.playing ?? false;
          if (isPlaying) { if (!_vinylController.isAnimating) _vinylController.repeat(); }
          else { _vinylController.stop(); }
        });

        return PopScope(
          canPop: false,
          onPopInvoked: (didPop) async {
            if (didPop) return;
            
            if (_pageController.hasClients) {
              if (_currentPage > 1) {
                // 如果在歌词页，先回到唱片页
                _pageController.animateToPage(1, duration: const Duration(milliseconds: 300), curve: Curves.easeOut);
                return;
              } else if (_currentPage == 0) {
                 // 如果在推荐页，回到唱片页
                _pageController.animateToPage(1, duration: const Duration(milliseconds: 300), curve: Curves.easeOut);
                return;
              }
            }
            
            // 切换 Tab 索引
            ref.read(navigationIndexProvider.notifier).state = 0;
          },
          child: Scaffold(
            backgroundColor: Colors.transparent,
            body: Stack(
            children: [
// ... (omitting some lines for brevity in search but providing context in new_string)
// Wait, the replace tool needs exact context. I should be careful.
              // This part will no longer rebuild every 100ms
              RepaintBoundary(
                child: Stack(
                  children: [
                    Positioned.fill(child: mediaItem.artUri != null ? CachedNetworkImage(imageUrl: mediaItem.artUri.toString(), fit: BoxFit.cover) : Container(color: const Color(0xFF0A0A12))),
                    Positioned.fill(child: BackdropFilter(filter: ImageFilter.blur(sigmaX: 80, sigmaY: 80), child: Container(color: Colors.black.withOpacity(0.6)))),
                  ],
                ),
              ),
              
              SafeArea(
                bottom: false,
                child: Stack(
                  children: [
                    Column(
                      children: [
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 8),
                          child: Row(
                            children: [
                              IconButton(
                                icon: const Icon(Icons.keyboard_arrow_down_rounded, size: 32, color: Colors.white70),
                                onPressed: () => ref.read(navigationIndexProvider.notifier).state = 0,
                              ),
                            ],
                          ),
                        ),
                        Expanded(
                          child: PageView(
                            key: ValueKey(mediaItem.id), // 换歌时强制重置 PageView 状态
                            controller: _pageController,
                            physics: const BouncingScrollPhysics(), // 强制开启
                            onPageChanged: (idx) {
                              if (mounted) setState(() => _currentPage = idx);
                            },
                            children: [
                              _RecommendationsView(mediaItem: mediaItem),
                              _MainVinylView(mediaItem: mediaItem, rotationAnimation: _vinylController, artistName: artistName, albumTitle: albumTitle),
                              _FullLyricsView(onUserScroll: _handleUserScroll),
                            ],
                          ),
                        ),
                      ],
                    ),
                    Positioned(
                      bottom: 0, left: 0, right: 0,
                      child: _PlaybackControlsContainer(
                        currentPage: _currentPage, 
                        mediaItem: mediaItem,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    },
      loading: () => const Scaffold(backgroundColor: Colors.transparent, body: Center(child: CircularProgressIndicator())),
      error: (e, _) => Scaffold(backgroundColor: Colors.transparent, body: Center(child: Text('加载播放器失败: $e', style: const TextStyle(color: Colors.white24)))),
    );
  }

  Widget _buildEmptyState() {
    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: RadialGradient(
            colors: [Color(0xFF1E293B), Color(0xFF05070A)],
            center: Alignment.center,
            radius: 1.5,
          ),
        ),
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                padding: const EdgeInsets.all(24),
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: Colors.white.withOpacity(0.03),
                ),
                child: Icon(Icons.music_note_rounded, size: 64, color: Colors.white.withOpacity(0.1)),
              ),
              const SizedBox(height: 32),
              Text('静待音律', style: GoogleFonts.montserrat(color: Colors.white.withOpacity(0.6), fontSize: 20, fontWeight: FontWeight.w900, letterSpacing: 1.2)),
              const SizedBox(height: 12),
              Text('在首页挑选一首动听的歌曲开启旅程', style: TextStyle(color: Colors.white.withOpacity(0.2), fontSize: 13, letterSpacing: 0.5)),
            ],
          ),
        ),
      ),
    );
  }
}

class _PlaybackControlsContainer extends ConsumerWidget {
  final int currentPage; 
  final MediaItem mediaItem;

  const _PlaybackControlsContainer({
    required this.currentPage, 
    required this.mediaItem,
    super.key,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Watch high-frequency state here, locally
    final playbackState = ref.watch(playbackStateProvider).value;

    return AnimatedSlide(
      duration: const Duration(milliseconds: 300),
      offset: currentPage == 1 ? Offset.zero : const Offset(0, 1),
      child: AnimatedOpacity(
        duration: const Duration(milliseconds: 300),
        opacity: currentPage == 1 ? 1 : 0,
        child: IgnorePointer(
          ignoring: currentPage != 1, 
          child: Container(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                colors: [Colors.transparent, Colors.black.withOpacity(0.6)],
                begin: Alignment.topCenter, end: Alignment.bottomCenter
              ),
            ),
            child: _PlaybackControls(
              playbackState: playbackState, 
              mediaItem: mediaItem,
            ),
          )
        ),
      ),
    );
  }
}

class _MainVinylView extends StatelessWidget {
  final MediaItem mediaItem; final Animation<double> rotationAnimation;
  final String artistName; final String albumTitle;
  const _MainVinylView({required this.mediaItem, required this.rotationAnimation, required this.artistName, required this.albumTitle});
  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final screenHeight = constraints.maxHeight;
        final screenWidth = constraints.maxWidth;
        // 动态计算唱片大小：在小屏幕上缩小比例，确保不溢出
        final vinylSize = screenHeight < 600 ? screenWidth * 0.55 : screenWidth * 0.70;
        
        return SingleChildScrollView(
          physics: const BouncingScrollPhysics(),
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: screenHeight),
            child: IntrinsicHeight(
              child: Column(
                children: [
                  SizedBox(height: screenHeight * 0.05), 
                  
                  RepaintBoundary(
                    child: RotationTransition(
                      turns: rotationAnimation,
                      child: Center(
                        child: Container(
                          width: vinylSize, height: vinylSize, padding: const EdgeInsets.all(8),
                          decoration: BoxDecoration(shape: BoxShape.circle, border: Border.all(color: Colors.white10, width: 0.5), gradient: const SweepGradient(colors: [Colors.black, Color(0xFF1A1A1A), Colors.black, Color(0xFF2A2A2A), Colors.black], stops: [0.0, 0.25, 0.5, 0.75, 1.0]), boxShadow: const [BoxShadow(color: Colors.black54, blurRadius: 40, spreadRadius: 10)]),
                          child: Container(decoration: const BoxDecoration(shape: BoxShape.circle, color: Colors.black), child: ClipOval(child: CachedNetworkImage(imageUrl: mediaItem.artUri.toString(), fit: BoxFit.cover, placeholder: (c, u) => const Icon(Icons.music_note, size: 80, color: Colors.white10), errorWidget: (c, u, e) => const Icon(Icons.music_note, size: 80, color: Colors.white10)))),
                        ),
                      ),
                    ),
                  ),
                  
                  const Spacer(flex: 2), 
                  
                  RepaintBoundary(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 40),
                      child: Column(
                        children: [
                          Text(mediaItem.title, style: const TextStyle(fontSize: 26, fontWeight: FontWeight.w900, color: Colors.white, letterSpacing: -0.5), textAlign: TextAlign.center, maxLines: 1, overflow: TextOverflow.ellipsis),
                          const SizedBox(height: 8),
                          Text("$artistName — $albumTitle", style: TextStyle(fontSize: 15, color: Colors.white.withOpacity(0.5), fontWeight: FontWeight.w500), textAlign: TextAlign.center, maxLines: 1, overflow: TextOverflow.ellipsis),
                        ],
                      ),
                    ),
                  ),
                  
                  const SizedBox(height: 24), 
                  const _SingleLineLyricPreview(),
                  
                  const Spacer(flex: 4),
                  // 底部安全避让
                  const SizedBox(height: 180), 
                ],
              ),
            ),
          ),
        );
      }
    );
  }
}

class _SingleLineLyricPreview extends ConsumerWidget {
  const _SingleLineLyricPreview();
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final lyrics = ref.watch(rawLyricsProvider).value;
    final activeIndex = ref.watch(currentLyricIndexProvider);
    if (lyrics == null || activeIndex < 0 || activeIndex >= lyrics.length) return const SizedBox(height: 40);
    final line = lyrics[activeIndex];
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 40),
      child: Column(
        children: [
          Text(line.text, textAlign: TextAlign.center, style: const TextStyle(fontSize: 18, color: Colors.white, fontWeight: FontWeight.bold)),
          if (line.translation != null) Padding(padding: const EdgeInsets.only(top: 4), child: Text(line.translation!, textAlign: TextAlign.center, style: TextStyle(fontSize: 14, color: Colors.white.withOpacity(0.5)))),
        ],
      ),
    );
  }
}

class _FullLyricsView extends ConsumerStatefulWidget {
  final VoidCallback onUserScroll;
  const _FullLyricsView({required this.onUserScroll, super.key});
  @override
  ConsumerState<_FullLyricsView> createState() => _FullLyricsViewState();
}

class _FullLyricsViewState extends ConsumerState<_FullLyricsView> {
  final ItemScrollController _itemScrollController = ItemScrollController();
  final ItemPositionsListener _itemPositionsListener = ItemPositionsListener.create();
  bool _isUserScrolling = false;
  Timer? _userScrollTimer;
  int _focusedIndex = -1; // 当前滑动焦点所在的行

  @override
  void initState() {
    super.initState();
    _itemPositionsListener.itemPositions.addListener(_updateFocusedIndex);
  }

  void _updateFocusedIndex() {
    if (!_isUserScrolling) return;
    
    final positions = _itemPositionsListener.itemPositions.value;
    if (positions.isEmpty) return;

    // 找到最接近屏幕中心 (alignment 0.35) 的项
    // ItemPosition.itemLeadingEdge 0.0 表示顶部，1.0 表示底部
    // 我们寻找最靠近 0.35 的那一项
    const targetAlignment = 0.35;
    
    int? closestIndex;
    double minDistance = double.infinity;

    for (final pos in positions) {
      final distance = (pos.itemLeadingEdge - targetAlignment).abs();
      if (distance < minDistance) {
        minDistance = distance;
        closestIndex = pos.index;
      }
    }

    if (closestIndex != null && closestIndex != _focusedIndex) {
      setState(() => _focusedIndex = closestIndex!);
      HapticFeedback.selectionClick();
    }
  }

  void _scrollToIndex(int index, {bool immediate = false}) {
    if (index < 0 || !_itemScrollController.isAttached || _isUserScrolling) return;
    
    if (immediate) {
      // 即使是初始定位，也给一个极短的过渡 (150ms)，消除硬生生的闪现感
      _itemScrollController.scrollTo(
        index: index,
        duration: const Duration(milliseconds: 150),
        curve: Curves.easeOut,
        alignment: 0.35,
      );
    } else {
      _itemScrollController.scrollTo(
        index: index,
        duration: const Duration(milliseconds: 800), // 显著增加时长，更加丝滑
        curve: Curves.easeInOutCubic, // 使用更优雅的缓动曲线
        alignment: 0.35,
      );
    }
  }

  void _onUserInteraction() {
    if (_userScrollTimer?.isActive ?? false) _userScrollTimer!.cancel();
    if (!_isUserScrolling) {
      setState(() => _isUserScrolling = true);
    }
    
    _userScrollTimer = Timer(const Duration(seconds: 5), () {
      if (mounted) {
        setState(() {
          _isUserScrolling = false;
          _focusedIndex = -1;
        });
        // 恢复后平滑滚动到当前活跃行
        final activeIndex = ref.read(currentLyricIndexProvider);
        _scrollToIndex(activeIndex);
      }
    });
  }

  @override
  void dispose() {
    _itemPositionsListener.itemPositions.removeListener(_updateFocusedIndex);
    _userScrollTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Consumer(
      builder: (context, ref, _) {
        final lyricsAsync = ref.watch(rawLyricsProvider);
        final activeIndex = ref.watch(currentLyricIndexProvider);
        
        // 核心修复 1：在构建完成后的下一帧，如果不是用户正在滚动，则立即同步到当前行
        // 这解决了进入歌词页时，若歌词索引未发生变化则不滚动的 Bug
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && !_isUserScrolling && activeIndex != -1) {
            _scrollToIndex(activeIndex, immediate: true);
          }
        });

        // 核心修复 2：只有当索引真正变化时才进行平滑滚动 (且非用户操作中)
        ref.listen(currentLyricIndexProvider, (prev, next) {
           if (next != prev && next != -1 && !_isUserScrolling) {
             _scrollToIndex(next);
           }
        });

        // 核心修复 2：监听歌曲变化进行瞬间重置
        ref.listen(currentTrackProvider, (prev, next) {
          final prevId = prev?.value?.id;
          final nextId = next.value?.id;
          if (nextId != null && nextId != prevId) {
            _scrollToIndex(0, immediate: true);
          }
        });

        return lyricsAsync.when(
          data: (lyrics) => lyrics == null ? const Center(child: Text('暂无歌词', style: TextStyle(color: Colors.white24)))
            : Stack(
                children: [
                  GestureDetector(
                    onLongPress: () {
                      // AI Calibration removed
                    },
                    child: NotificationListener<ScrollNotification>(
                      onNotification: (notification) {
                        if (notification is ScrollUpdateNotification && notification.dragDetails != null) {
                          _onUserInteraction();
                        }
                        return false;
                      },
                      child: ScrollablePositionedList.builder(
                        itemScrollController: _itemScrollController,
                        itemPositionsListener: _itemPositionsListener,
                        padding: EdgeInsets.fromLTRB(32, MediaQuery.of(context).size.height * 0.3, 32, MediaQuery.of(context).size.height * 0.4), 
                        itemCount: lyrics.length,
                        itemBuilder: (c, i) {
                        final line = lyrics[i]; 
                        final isActive = i == activeIndex;
                        final isFocused = i == _focusedIndex;
                        final hasTranslation = line.translation != null;
                        final displayActive = isActive || (_isUserScrolling && isFocused);

                        return AnimatedContainer(
                          duration: const Duration(milliseconds: 500), // 增加文字过渡时长，产生呼吸感
                          curve: Curves.easeInOut,
                          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 20),
                          alignment: Alignment.center,
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Text(
                                line.text, 
                                textAlign: TextAlign.center, 
                                style: TextStyle(
                                  fontSize: 22, 
                                  fontWeight: displayActive ? FontWeight.bold : FontWeight.w400, 
                                  color: displayActive 
                                    ? Colors.white.withOpacity(isFocused ? 0.8 : 1.0)
                                    : Colors.white.withOpacity(0.3)
                                )
                              ),
                              if (hasTranslation) 
                                Padding(
                                  padding: const EdgeInsets.only(top: 10), 
                                  child: Text(
                                    line.translation!, 
                                    textAlign: TextAlign.center, 
                                    style: TextStyle(
                                      fontSize: displayActive ? 16 : 14, 
                                      color: displayActive ? Colors.white70 : Colors.white.withOpacity(0.2)
                                    )
                                  )
                                ),
                            ],
                          ),
                        );
                      },
                    ),
                  ),
                ),
                
                // QQ 音乐风格：滑动时的播放指示器
                if (_isUserScrolling && _focusedIndex != -1)
                  Positioned(
                    top: MediaQuery.of(context).size.height * 0.35 + 50, // 对应 alignment 0.35
                    left: 0,
                    right: 0,
                    child: IgnorePointer(
                      ignoring: false,
                      child: Row(
                        children: [
                          IconButton(
                            icon: const Icon(Icons.play_arrow_rounded, color: Colors.white70),
                            onPressed: () {
                              final line = lyrics[_focusedIndex];
                              ref.read(playerControllerProvider)?.seek(Duration(milliseconds: (line.time * 1000).toInt()));
                              setState(() {
                                _isUserScrolling = false;
                                _focusedIndex = -1;
                              });
                            },
                          ),
                          Expanded(
                            child: Container(
                              height: 1,
                              decoration: BoxDecoration(
                                gradient: LinearGradient(
                                  colors: [Colors.white24, Colors.white.withOpacity(0.05)],
                                )
                              ),
                            ),
                          ),
                          Padding(
                            padding: const EdgeInsets.only(right: 16, left: 8),
                            child: Text(
                              _formatTime(lyrics[_focusedIndex].time),
                              style: const TextStyle(color: Colors.white38, fontSize: 10, fontFamily: 'monospace'),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
              ],
            ),
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (_, __) => const Center(child: Icon(Icons.error_outline)),
        );
      }
    );
  }

  String _formatTime(double seconds) {
    final m = (seconds / 60).floor();
    final s = (seconds % 60).floor();
    return '${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
  }
}

class _RecommendationsView extends ConsumerStatefulWidget {
  final MediaItem mediaItem;
  const _RecommendationsView({required this.mediaItem});
  @override
  ConsumerState<_RecommendationsView> createState() => _RecommendationsViewState();
}

class _RecommendationsViewState extends ConsumerState<_RecommendationsView> {
  Future<List<dynamic>>? _recommendationsFuture;
  
  @override
  void initState() { 
    super.initState(); 
    // Delay loading until transition finishes to improve performance
    Future.delayed(const Duration(milliseconds: 500), () {
      if (mounted) _loadRecommendations();
    });
  }

  void _loadRecommendations() {
    setState(() {
      final artistId = widget.mediaItem.extras?['artistId'];
      _recommendationsFuture = Future.wait([
        if (artistId != null) ref.read(musicRepositoryProvider).getArtistTracks(artistId),
        ref.read(musicRepositoryProvider).getTracks(limit: 15),
      ]).then((results) {
        final List<dynamic> tracks = (artistId != null && results.isNotEmpty) ? [...(results[0] as List)] : [];
        final List<dynamic> pool = (results.length > 1) ? results[1] as List : (results.isNotEmpty ? results[0] as List : []);
        tracks.removeWhere((t) => t.id == widget.mediaItem.id);
        if (tracks.length < 6) {
          final fallback = pool.where((t) => !tracks.any((rt) => rt.id == t.id) && t.id != widget.mediaItem.id).toList();
          fallback.shuffle(); tracks.addAll(fallback);
        }
        return tracks.take(15).toList();
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    if (_recommendationsFuture == null) return const Center(child: CircularProgressIndicator());
    final config = ref.watch(envConfigNotifierProvider);
    final artistId = widget.mediaItem.extras?['artistId'];
    final albumId = widget.mediaItem.extras?['albumId'];
    final artistName = widget.mediaItem.extras?['artistName'] ?? widget.mediaItem.artist ?? '未知歌手';
    final albumTitle = widget.mediaItem.extras?['albumTitle'] ?? widget.mediaItem.album ?? '未知专辑';
    final artistImageUrlRaw = widget.mediaItem.extras?['artistImageUrl'];

    final effectiveArtistImageUrl = config.getEffectiveImageUrl(artistImageUrlRaw);

    return ListView(
      physics: const BouncingScrollPhysics(), padding: const EdgeInsets.fromLTRB(24, 20, 24, 180), // 增加底部边距防止被控制栏遮挡
      children: [
        const Text('当前旋律', style: TextStyle(fontSize: 12, color: Colors.white38, fontWeight: FontWeight.bold, letterSpacing: 1.5)),
        const SizedBox(height: 20),
        Row(children: [
          Expanded(child: _EntryCard(title: artistName, subtitle: '歌手详情', imageUrl: effectiveArtistImageUrl, icon: Icons.person_rounded, onTap: () async {
            if (artistId != null) {
              final artist = await ref.read(musicRepositoryProvider).getArtistById(artistId);
              if (context.mounted) Navigator.push(context, MaterialPageRoute(builder: (c) => ArtistDetailView(artist: artist)));
            }
          })),
          const SizedBox(width: 16),
          Expanded(child: _EntryCard(title: albumTitle, subtitle: '专辑详情', imageUrl: albumId != null ? '${config.coversBaseUrl}$albumId' : null, icon: Icons.album_rounded, onTap: () async {
            if (albumId != null) {
              final album = await ref.read(musicRepositoryProvider).getAlbumById(albumId);
              if (context.mounted) Navigator.push(context, MaterialPageRoute(builder: (c) => MusicDetailView(item: album)));
            }
          })),
        ]),
        const SizedBox(height: 40),
        const Row(children: [Icon(Icons.auto_awesome_motion_rounded, color: Color(0xFF38BDF8), size: 20), SizedBox(width: 8), Text('相似推荐', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold, color: Colors.white))]),
        const SizedBox(height: 20),
        FutureBuilder<List<dynamic>>(
          future: _recommendationsFuture,
          builder: (context, snapshot) {
            if (snapshot.connectionState == ConnectionState.waiting) return const Center(child: CircularProgressIndicator());
            final tracks = snapshot.data ?? [];
            return Column(children: tracks.map((t) {
              final coverUrl = t.albumId != null ? '${config.coversBaseUrl}${t.albumId}' : null;
              final tArtistName = t.artistName ?? 'Papilio Artist'; 
              return Container(
                margin: const EdgeInsets.only(bottom: 12), decoration: BoxDecoration(color: Colors.white.withOpacity(0.05), borderRadius: BorderRadius.circular(16)),
                child: ListTile(
                  contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
                  leading: Container(width: 44, height: 44, decoration: BoxDecoration(borderRadius: BorderRadius.circular(8), color: Colors.white10), child: coverUrl != null ? ClipRRect(borderRadius: BorderRadius.circular(8), child: CachedNetworkImage(imageUrl: coverUrl, fit: BoxFit.cover)) : const Icon(Icons.music_note_rounded)),
                  title: Text(t.title, style: const TextStyle(fontWeight: FontWeight.bold, color: Colors.white, fontSize: 14), maxLines: 1, overflow: TextOverflow.ellipsis),
                  subtitle: Text(tArtistName, style: const TextStyle(color: Colors.white54, fontSize: 12)),
                  onTap: () => ref.read(playerControllerProvider)?.playTrack(t),
                ),
              );
            }).toList());
          },
        ),
      ],
    );
  }
}

class _EntryCard extends StatelessWidget {
  final String title; final String subtitle; final String? imageUrl; final IconData icon; final VoidCallback onTap;
  const _EntryCard({required this.title, required this.subtitle, this.imageUrl, required this.icon, required this.onTap});
  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap, 
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 16), 
        decoration: BoxDecoration(color: Colors.white.withOpacity(0.08), borderRadius: BorderRadius.circular(24), border: Border.all(color: Colors.white10)), 
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(width: 50, height: 50, decoration: const BoxDecoration(shape: BoxShape.circle, color: Colors.white10), child: imageUrl != null ? ClipOval(child: CachedNetworkImage(imageUrl: imageUrl!, fit: BoxFit.cover, alignment: Alignment.topCenter)) : Icon(icon, color: Colors.white38, size: 24)), 
            const SizedBox(height: 12), 
            Text(title, style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13, color: Colors.white), maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.center), 
            const SizedBox(height: 4),
            Text(subtitle, style: const TextStyle(fontSize: 10, color: Color(0xFF38BDF8), fontWeight: FontWeight.bold), maxLines: 1, overflow: TextOverflow.ellipsis)
          ]
        )
      )
    );
  }
}

class _PlaybackControls extends ConsumerStatefulWidget {
  final PlaybackState? playbackState; 
  final MediaItem mediaItem;

  const _PlaybackControls({
    required this.playbackState, 
    required this.mediaItem,
    super.key,
  });

  @override
  ConsumerState<_PlaybackControls> createState() => _PlaybackControlsState();
}

class _PlaybackControlsState extends ConsumerState<_PlaybackControls> {
  bool? _optimisticFavorite;
  AudioServiceRepeatMode? _optimisticRepeatMode;

  @override
  Widget build(BuildContext context) {
    final playbackState = widget.playbackState;
    final mediaItem = widget.mediaItem;
    
    // High-precision progress calculation
    final currentPos = playbackState?.position ?? Duration.zero;
    final lastUpdate = playbackState?.updateTime ?? DateTime.now();
    final isPlaying = playbackState?.playing ?? false;
    
    final isFavorite = _optimisticFavorite ?? (mediaItem.extras?['isFavorite'] == true);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 20),
      child: Column(children: [
        // ProgressBar rebuilt locally via Consumer
        Consumer(
          builder: (context, ref, _) {
            // Force progress bar tick - Using dedicated provider to prevent leaks
            ref.watch(playbackProgressTickerProvider);
            final elapsed = isPlaying ? DateTime.now().difference(lastUpdate) : Duration.zero;
            final interpolatedPos = currentPos + elapsed;
            
            return ProgressBar(
              progress: interpolatedPos, 
              total: mediaItem.duration ?? Duration.zero,
              onSeek: (d) => ref.read(playerControllerProvider)?.seek(d),
              progressBarColor: Theme.of(context).colorScheme.primary, 
              baseBarColor: Colors.white10, 
              thumbColor: Colors.white,
              timeLabelTextStyle: const TextStyle(color: Colors.white54, fontSize: 12),
            );
          },
        ),
        const SizedBox(height: 10),
        Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
          _RepeatModeButton(state: playbackState),
          IconButton(icon: const Icon(Icons.skip_previous_rounded, size: 40, color: Colors.white), onPressed: () => ref.read(playerControllerProvider)?.skipToPrevious()),
          GestureDetector(onTap: () => ref.read(playerControllerProvider)?.togglePlay(), child: Container(width: 76, height: 76, decoration: const BoxDecoration(color: Colors.white, shape: BoxShape.circle), child: Icon(playbackState?.playing == true ? Icons.pause_rounded : Icons.play_arrow_rounded, size: 48, color: Colors.black))),
          IconButton(icon: const Icon(Icons.skip_next_rounded, size: 40, color: Colors.white), onPressed: () => ref.read(playerControllerProvider)?.skipToNext()),
          _ShuffleModeButton(state: playbackState),
        ]),
        const SizedBox(height: 10),
        Row(mainAxisAlignment: MainAxisAlignment.center, children: [
          IconButton(
            icon: Icon(isFavorite ? Icons.favorite_rounded : Icons.favorite_border_rounded, 
            color: isFavorite ? Colors.redAccent : Colors.white70, size: 24), 
            onPressed: () {
              setState(() => _optimisticFavorite = !isFavorite);
              ref.read(musicRepositoryProvider).toggleFavorite(mediaItem.id);
            }
          ),
          const SizedBox(width: 24),
          /* IconButton(
            icon: const Icon(Icons.tune_rounded, color: Colors.white70, size: 24), 
            onPressed: () {
              // Calibration removed
            }
          ), */
          const SizedBox(width: 24),
          IconButton(
            icon: const Icon(Icons.playlist_play_rounded, color: Colors.white70, size: 24), 
            onPressed: () => showModalBottomSheet(context: context, backgroundColor: Colors.black87, isScrollControlled: true, shape: const RoundedRectangleBorder(borderRadius: BorderRadius.vertical(top: Radius.circular(24))), builder: (context) => const _QueueDrawer())
          ),
          const SizedBox(width: 24),
          _DownloadButton(mediaItem: mediaItem),
        ]),
      ]),
    );
  }
}

class _DownloadButton extends ConsumerWidget {
  final MediaItem mediaItem;
  const _DownloadButton({required this.mediaItem});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final downloadState = ref.watch(downloadProvider);
    final progress = downloadState.progress[mediaItem.id];
    final isDownloading = progress != null && progress < 1.0;
    final isDownloaded = progress == 1.0;

    if (isDownloading) {
      return SizedBox(
        width: 24, height: 24,
        child: CircularProgressIndicator(
          value: progress,
          strokeWidth: 2,
          color: Theme.of(context).colorScheme.primary,
        ),
      );
    }

    return IconButton(
      icon: Icon(
        isDownloaded ? Icons.check_circle_outline_rounded : Icons.download_for_offline_rounded, 
        color: isDownloaded ? Theme.of(context).colorScheme.secondary : Colors.white70, 
        size: 24
      ), 
      onPressed: isDownloaded ? null : () {
        try {
          // 直接构建 Track 骨架进行下载，ID 是唯一关键点，避免报错的 getTrackById 调用
          final track = Track(
            id: mediaItem.id,
            title: mediaItem.title,
            albumId: mediaItem.extras?['albumId'],
            artistId: mediaItem.extras?['artistId'],
            artistName: mediaItem.extras?['artistName'] ?? mediaItem.artist,
            albumTitle: mediaItem.extras?['albumTitle'] ?? mediaItem.album,
            duration: mediaItem.duration?.inSeconds ?? 0,
          );
          ref.read(downloadProvider.notifier).download(track);
        } catch (e) {
          if (context.mounted) {
            ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('启动下载失败: $e')));
          }
        }
      }
    );
  }
}

class _RepeatModeButton extends ConsumerWidget {
  final PlaybackState? state;
  const _RepeatModeButton({required this.state});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final repeatMode = state?.repeatMode ?? AudioServiceRepeatMode.none;
    
    final (icon, color) = switch (repeatMode) {
      AudioServiceRepeatMode.one => (Icons.repeat_one_rounded, Theme.of(context).colorScheme.primary),
      AudioServiceRepeatMode.all => (Icons.repeat_rounded, Theme.of(context).colorScheme.primary),
      _ => (Icons.repeat_rounded, Colors.white38),
    };

    return IconButton(
      icon: Icon(icon, size: 22, color: color),
      onPressed: () => ref.read(playerControllerProvider)?.toggleRepeatMode(),
    );
  }
}

class _ShuffleModeButton extends ConsumerWidget {
  final PlaybackState? state;
  const _ShuffleModeButton({required this.state});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // 工业级同步：直接观察播放状态流，不依赖传递的快照
    final currentState = ref.watch(playbackStateProvider).value;
    final shuffleMode = currentState?.shuffleMode ?? AudioServiceShuffleMode.none;
    final isActive = shuffleMode == AudioServiceShuffleMode.all;

    return IconButton(
      icon: Icon(
        isActive ? Icons.shuffle_on_rounded : Icons.shuffle_rounded, 
        size: 22, 
        color: isActive ? Theme.of(context).colorScheme.primary : Colors.white38
      ),
      onPressed: () {
        debugPrint('Toggling shuffle: current is $shuffleMode');
        ref.read(playerControllerProvider)?.toggleShuffleMode();
      },
    );
  }
}

class _QueueDrawer extends ConsumerWidget {
  const _QueueDrawer();
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final qAsync = ref.watch(queueProvider);
    final curr = ref.watch(currentTrackProvider).value;
    return Container(
      height: MediaQuery.of(context).size.height * 0.75,
      decoration: BoxDecoration(
        color: const Color(0xFF0F172A).withOpacity(0.95),
        borderRadius: const BorderRadius.vertical(top: Radius.circular(32)),
        border: Border.all(color: Colors.white10),
      ),
      child: Stack(
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 32),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    const Text('播放队列', style: TextStyle(fontSize: 22, fontWeight: FontWeight.bold, color: Colors.white)),
                    Text('${qAsync.value?.length ?? 0} 首曲目', style: const TextStyle(color: Colors.white54, fontSize: 14)),
                  ],
                ),
                const SizedBox(height: 24),
                Expanded(
                  child: qAsync.when(
                    data: (q) => ListView.builder(
                      itemCount: q.length, 
                      physics: const BouncingScrollPhysics(),
                      itemBuilder: (c, i) {
                        final isPlaying = q[i].id == curr?.id;
                        return ListTile(
                          contentPadding: EdgeInsets.zero,
                          leading: isPlaying 
                            ? SizedBox(width: 32, child: Center(child: PlayingVisualizer(color: Theme.of(context).colorScheme.primary, size: 16))) 
                            : SizedBox(width: 32, child: Text('${i + 1}', style: const TextStyle(color: Colors.white24, fontSize: 14), textAlign: TextAlign.center)),
                          title: Text(q[i].title, style: TextStyle(color: isPlaying ? Theme.of(context).colorScheme.primary : Colors.white, fontWeight: isPlaying ? FontWeight.bold : FontWeight.normal), maxLines: 1, overflow: TextOverflow.ellipsis),
                          subtitle: Text(q[i].artist ?? '未知艺术家', style: const TextStyle(color: Colors.white54, fontSize: 12)),
                          onTap: () { ref.read(playerControllerProvider)?.skipToQueueItem(i); Navigator.pop(context); },
                        );
                      },
                    ),
                    loading: () => const Center(child: CircularProgressIndicator()),
                    error: (e, _) => const Center(child: Text('无法加载队列')),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}