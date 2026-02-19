import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:palette_generator/palette_generator.dart';
import 'player_provider.dart';

final dynamicThemeProvider = StateNotifierProvider<DynamicThemeNotifier, Color>((ref) {
  return DynamicThemeNotifier(ref);
});

class DynamicThemeNotifier extends StateNotifier<Color> {
  final Ref _ref;
  static const Color defaultAccent = Color(0xFF8B5CF6); // Papilio Purple

  DynamicThemeNotifier(this._ref) : super(defaultAccent) {
    _listenToTrackChanges();
  }

  void _listenToTrackChanges() {
    _ref.listen(currentTrackProvider, (previous, next) async {
      final mediaItem = next.value;
      if (mediaItem?.artUri == null) {
        state = defaultAccent;
        return;
      }

      try {
        final imageProvider = NetworkImage(mediaItem!.artUri.toString());
        final palette = await PaletteGenerator.fromImageProvider(
          imageProvider,
          maximumColorCount: 10,
        );
        
        if (palette.dominantColor != null) {
          // Adjust brightness/saturation if needed to ensure contrast
          state = palette.dominantColor!.color;
        }
      } catch (e) {
        state = defaultAccent;
      }
    });
  }
}
