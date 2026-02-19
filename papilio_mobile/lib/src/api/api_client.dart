import 'package:flutter/foundation.dart';
import 'package:dio/dio.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../config/env_config.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../providers/auth_provider.dart';

part 'api_client.g.dart';

@riverpod
ApiClient apiClient(ApiClientRef ref) {
  final config = ref.watch(envConfigNotifierProvider);
  return ApiClient(config, ref);
}

class ApiClient {
  final EnvConfig _config;
  final ApiClientRef _ref; 
  late final Dio _dio;
  int _consecutiveErrors = 0;

  ApiClient(this._config, this._ref) {
    _dio = Dio(BaseOptions(
      baseUrl: _config.apiBaseUrl,
      connectTimeout: const Duration(seconds: 10),
      receiveTimeout: const Duration(seconds: 15),
    ));

    _dio.interceptors.add(InterceptorsWrapper(
      onRequest: (options, handler) async {
        // 白名单：登录和注册不需要 Token
        if (options.path.contains('auth/login') || options.path.contains('auth/register')) {
          return handler.next(options);
        }

        final prefs = await SharedPreferences.getInstance();
        final token = prefs.getString('auth_token');
        
        if (token == null) {
          debugPrint('SilentGuard: Intercepting unauthenticated request to ${options.path}');
          return handler.reject(
            DioException(
              requestOptions: options,
              type: DioExceptionType.cancel,
              error: 'Authentication missing - Silenced to prevent dialog storm',
            ),
            true, 
          );
        }

        options.headers['Authorization'] = 'Bearer $token';
        return handler.next(options);
      },
      onError: (e, handler) {
        // 如果是由于我们主动取消（无 Token），直接向上传递该错误，不触发全局弹窗逻辑
        if (e.type == DioExceptionType.cancel) {
          return handler.next(e);
        }

        // 核心加固：处理 401 未授权错误，自动重置登录状态
        if (e.response?.statusCode == 401) {
          debugPrint('AuthGuard: 401 Unauthorized detected at ${e.requestOptions.path}. Triggering logout.');
          // 使用 ref 动态更新状态（注意：需要确保 authStateProvider 已就绪）
          _ref.read(authStateProvider.notifier).setLoggedIn(false);
        }

        _consecutiveErrors++;
        return handler.next(e);
      },
      onResponse: (r, handler) {
        _consecutiveErrors = 0;
        return handler.next(r);
      }
    ));
  }

  String get apiBaseUrl => _config.apiBaseUrl;
  Dio get dioInstance => _dio;

  Future<Response> get(String path, {Map<String, dynamic>? queryParameters}) => _dio.get(path, queryParameters: queryParameters);
  Future<Response> post(String path, {dynamic data}) => _dio.post(path, data: data);
  Future<Response> patch(String path, {dynamic data}) => _dio.patch(path, data: data);
  Future<Response> delete(String path) => _dio.delete(path);
}
