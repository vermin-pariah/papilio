import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../api/auth_repository.dart';

part 'auth_provider.g.dart';

@riverpod
class AuthState extends _$AuthState {
  @override
  bool? build() {
    checkAuth();
    return null;
  }

  Future<void> checkAuth() async {
    try {
      final repo = ref.read(authRepositoryProvider);
      final loggedIn = await repo.isLoggedIn().timeout(const Duration(seconds: 10));
      state = loggedIn;
    } catch (e) {
      debugPrint('Auth check transient failure: $e');
      state = false;
    }
  }

  void setLoggedIn(bool value) {
    state = value;
  }
}
