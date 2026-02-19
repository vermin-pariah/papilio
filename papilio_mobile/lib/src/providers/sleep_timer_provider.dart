import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'dart:async';
import 'player_provider.dart';

final sleepTimerProvider = StateNotifierProvider<SleepTimerNotifier, int?>((ref) {
  return SleepTimerNotifier(ref);
});

class SleepTimerNotifier extends StateNotifier<int?> {
  final Ref _ref;
  Timer? _timer;

  SleepTimerNotifier(this._ref) : super(null);

  void setTimer(int minutes) {
    _timer?.cancel();
    state = minutes * 60; // Convert to seconds
    
    _timer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (state == null) {
        timer.cancel();
        return;
      }
      
      if (state! <= 0) {
        _finish();
        timer.cancel();
      } else {
        state = state! - 1;
      }
    });
  }

  void cancelTimer() {
    _timer?.cancel();
    state = null;
  }

  void _finish() {
    state = null;
    _ref.read(playerControllerProvider)?.pause();
  }

  String get remainingLabel {
    if (state == null) return "未开启";
    final h = state! ~/ 3600;
    final m = (state! % 3600) ~/ 60;
    final s = state! % 60;
    if (h > 0) {
      return "${h.toString().padLeft(2, '0')}:${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}";
    }
    return "${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}";
  }
}
