import 'package:dio/dio.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../models/user.dart';
import 'api_client.dart';

part 'auth_repository.g.dart';

@riverpod
AuthRepository authRepository(AuthRepositoryRef ref) {
  final client = ref.watch(apiClientProvider);
  return AuthRepository(client);
}

@riverpod
Future<User?> currentUser(CurrentUserRef ref) async {
  final repo = ref.watch(authRepositoryProvider);
  return repo.getCurrentUser();
}

class AuthRepository {
  final ApiClient _client;

  AuthRepository(this._client);

  Future<bool> isLoggedIn() async {
    try {
      final response = await _client.get('auth/me');
      return response.statusCode == 200;
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return false;
      return false;
    } catch (_) {
      return false;
    }
  }

  Future<User?> getCurrentUser() async {
    try {
      final response = await _client.get('auth/me');
      return User.fromJson(response.data);
    } on DioException catch (e) {
      if (e.type == DioExceptionType.cancel) return null;
      rethrow;
    }
  }

  Future<Map<String, dynamic>> login(String username, String password) async {
    final response = await _client.post('auth/login', data: {
      'username': username,
      'password': password,
    });
    
    final data = response.data as Map<String, dynamic>;
    if (data.containsKey('token')) {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString('auth_token', data['token']);
    }
    
    return data;
  }

  Future<void> register(String username, String password, {String? nickname}) async {
    await _client.post('auth/register', data: {
      'username': username,
      'password': password,
      'nickname': nickname,
    });
  }

  Future<void> logout() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove('auth_token');
  }

  Future<void> updateProfile({String? nickname, String? email, String? password}) async {
    await _client.patch('auth/me', data: {
      if (nickname != null) 'nickname': nickname,
      if (email != null) 'email': email,
      if (password != null) 'password': password,
    });
  }

  Future<void> uploadAvatar(String filePath) async {
    final formData = FormData.fromMap({
      'avatar': await MultipartFile.fromFile(filePath),
    });
    await _client.dioInstance.post('auth/avatar', data: formData);
  }
}
