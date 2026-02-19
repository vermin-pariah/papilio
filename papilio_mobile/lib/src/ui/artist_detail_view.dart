import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:image_picker/image_picker.dart';
import '../api/music_repository.dart';
import '../providers/player_provider.dart';
import '../models/artist.dart';
import '../models/album.dart';
import '../models/track.dart';
import 'music_detail_view.dart';
import 'widgets/gradient_scaffold.dart';
import '../config/env_config.dart';
import 'widgets/track_action_sheet.dart';

class ArtistDetailView extends ConsumerWidget {
  final Artist artist;

  const ArtistDetailView({super.key, required this.artist});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final repository = ref.watch(musicRepositoryProvider);
    final config = ref.watch(envConfigNotifierProvider);
    
    final albumsFuture = repository.getArtistAlbums(artist.id);
    final tracksFuture = repository.getArtistTracks(artist.id);

    final effectiveArtistImageUrl = config.getEffectiveImageUrl(artist.imageUrl);

    return GradientScaffold(
      body: CustomScrollView(
        slivers: [
          SliverAppBar(
            expandedHeight: 250,
            pinned: true,
            actions: [
              PopupMenuButton<String>(
                icon: const Icon(Icons.more_vert_rounded),
                onSelected: (value) async {
                  final scaffoldMessenger = ScaffoldMessenger.of(context);
                  if (value == 'sync') {
                    try {
                      await repository.triggerArtistSyncSingle(artist.id);
                      scaffoldMessenger.showSnackBar(const SnackBar(content: Text('已触发同步任务，请稍后刷新')));
                    } catch (e) {
                      scaffoldMessenger.showSnackBar(SnackBar(content: Text('同步失败: $e')));
                    }
                  } else if (value == 'upload') {
                    final picker = ImagePicker();
                    final image = await picker.pickImage(source: ImageSource.gallery);
                    if (image != null) {
                      try {
                        await repository.uploadArtistAvatar(artist.id, image.path);
                        scaffoldMessenger.showSnackBar(const SnackBar(content: Text('头像上传成功，请刷新查看')));
                      } catch (e) {
                        scaffoldMessenger.showSnackBar(SnackBar(content: Text('上传失败: $e')));
                      }
                    }
                  }
                },
                itemBuilder: (context) => [
                  const PopupMenuItem(value: 'sync', child: ListTile(leading: Icon(Icons.sync_rounded), title: Text('同步元数据'), dense: true, contentPadding: EdgeInsets.zero)),
                  const PopupMenuItem(value: 'upload', child: ListTile(leading: Icon(Icons.upload_rounded), title: Text('上传头像'), dense: true, contentPadding: EdgeInsets.zero)),
                ],
              ),
              const SizedBox(width: 8),
            ],
            flexibleSpace: FlexibleSpaceBar(
              title: Text(artist.name, style: const TextStyle(fontWeight: FontWeight.bold)),
              background: effectiveArtistImageUrl != null 
                ? CachedNetworkImage(
                    imageUrl: effectiveArtistImageUrl, 
                    fit: BoxFit.cover,
                    alignment: Alignment.topCenter,
                  )
                : Container(
                    decoration: BoxDecoration(
                      gradient: LinearGradient(
                        colors: [Theme.of(context).colorScheme.primary, Theme.of(context).colorScheme.surface],
                        begin: Alignment.topLeft, end: Alignment.bottomRight
                      ),
                    ),
                    child: Icon(Icons.person_rounded, size: 100, color: Colors.white.withOpacity(0.1)),
                  ),
            ),
          ),

          if (artist.bio != null && artist.bio!.isNotEmpty)
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('关于', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
                    const SizedBox(height: 12),
                    Text(
                      artist.bio!,
                      style: TextStyle(color: Colors.white.withOpacity(0.7), height: 1.5),
                      maxLines: 5,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
            ),

          SliverToBoxAdapter(
            child: _SectionHeader(title: '专辑'),
          ),
          SliverToBoxAdapter(
            child: FutureBuilder<List<Album>>(
              future: albumsFuture,
              builder: (context, snapshot) {
                if (snapshot.connectionState == ConnectionState.waiting) return const Center(child: CircularProgressIndicator());
                final albums = snapshot.data ?? [];
                if (albums.isEmpty) return const Padding(padding: EdgeInsets.all(24), child: Text('暂无专辑'));
                return SizedBox(
                  height: 220,
                  child: ListView.builder(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    scrollDirection: Axis.horizontal,
                    itemCount: albums.length,
                    itemBuilder: (context, index) => _AlbumCard(album: albums[index]),
                  ),
                );
              },
            ),
          ),

          SliverToBoxAdapter(
            child: _SectionHeader(title: '所有单曲'),
          ),
          FutureBuilder<List<Track>>(
            future: tracksFuture,
            builder: (context, snapshot) {
              if (snapshot.connectionState == ConnectionState.waiting) return const SliverToBoxAdapter(child: Center(child: CircularProgressIndicator()));
              final tracks = snapshot.data ?? [];
              return SliverList(
                delegate: SliverChildBuilderDelegate(
                  (context, index) {
                    final track = tracks[index];
                    return ListTile(
                      contentPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 4),
                      title: Text(track.title, style: const TextStyle(fontWeight: FontWeight.bold)),
                      subtitle: Text(track.format ?? '未知格式'),
                      trailing: const Icon(Icons.more_vert_rounded),
                      onTap: () => ref.read(playerControllerProvider)?.playQueue(tracks, index),
                    );
                  },
                  childCount: tracks.length,
                ),
              );
            },
          ),
          
          SliverPadding(padding: EdgeInsets.only(bottom: MediaQuery.of(context).padding.bottom + 100)),
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
      padding: const EdgeInsets.fromLTRB(24, 32, 24, 16),
      child: Text(title, style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold)),
    );
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
      child: Container(
        width: 160,
        margin: const EdgeInsets.only(left: 8, right: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              height: 160,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(16),
                color: Colors.white10,
              ),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: CachedNetworkImage(
                  imageUrl: coverUrl,
                  fit: BoxFit.cover,
                  placeholder: (c, u) => Container(color: Colors.white10),
                  errorWidget: (c, u, e) => const Icon(Icons.album_rounded, size: 64, color: Colors.white10),
                ),
              ),
            ),
            const SizedBox(height: 8),
            Text(album.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontWeight: FontWeight.bold)),
            Text(album.releaseYear?.toString() ?? '未知年份', style: const TextStyle(fontSize: 12, color: Colors.white54)),
          ],
        ),
      ),
    );
  }
}
