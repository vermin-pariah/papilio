import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'dart:async';
import '../api/music_repository.dart';
import '../models/scan_status.dart';
import 'auth_provider.dart';

part 'scan_provider.g.dart';

@riverpod
Stream<ScanStatus> scanStatus(ScanStatusRef ref) {
  final isLoggedIn = ref.watch(authStateProvider);
  if (isLoggedIn != true) {
    return const Stream.empty();
  }

  final repository = ref.watch(musicRepositoryProvider);
  
  return Stream.periodic(const Duration(seconds: 5)).asyncMap((_) async {
    return await repository.getScanStatus();
  });
}

@riverpod
ScanController scanController(ScanControllerRef ref) {
  final repository = ref.watch(musicRepositoryProvider);
  return ScanController(repository, ref);
}

class ScanController {
  final MusicRepository _repository;
  final ScanControllerRef _ref;

  ScanController(this._repository, this._ref);

  Future<bool> startScan() async {
    try {
      await _repository.triggerScan();
      _ref.invalidate(scanStatusProvider);
      return true;
    } catch (e) {
      debugPrint('Scan trigger failed: $e');
      return false;
    }
  }
}