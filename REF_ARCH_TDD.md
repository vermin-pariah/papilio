# Papilio 2026 Refactoring & Upgrade Technical Design Document (TDD)

**Version**: 1.0
**Date**: 2026-02-13
**Author**: Architect (Gemini AI Chain)
**Status**: DRAFT

## 1. Executive Summary
This document outlines the architectural refactoring plan for the Papilio project to address aging dependencies and adopt modern best practices. The primary focus is upgrading the backend to **Axum 0.8** and the mobile client to **Riverpod 3.0** with code generation.

## 2. Backend Architecture (Rust)

### 2.1 Dependency Upgrades
| Crate | Current | Target | Notes |
| :--- | :--- | :--- | :--- |
| `axum` | 0.7 | 0.8.x | Major breaking changes in `Router` and `State` extraction. |
| `sqlx` | 0.8.0 | 0.8.x | Maintenance update for stability. |
| `tokio` | 1.x | 1.x (Latest) | Performance improvements. |
| `redis` | 0.27 | 1.0.x | Major version update. |

### 2.2 Architectural Changes
#### 2.2.1 Router State Management
**Current**: `Router::new()` implicitly creating `Router<()>` and merging state late via `.with_state()`.
**Refactoring**: 
- Explicitly type all router factory functions: `pub fn music_routes() -> Router<Arc<AppState>>`.
- Ensure strict type safety for `State` extraction in all handlers.
- Verify `tower-http` middleware compatibility with Axum 0.8 `Service` trait changes.

#### 2.2.2 Code Cleanup
- **Path Flattening**: Verify removal of `/v1/` is consistent in comments and code.
- **Error Handling**: Review `ApiError` to ensure it implements `IntoResponse` correctly for Axum 0.8.

## 3. Mobile Architecture (Flutter)

### 3.1 Dependency Upgrades
| Package | Current | Target | Notes |
| :--- | :--- | :--- | :--- |
| `flutter_riverpod` | ^2.4.9 | ^3.0.0 | Major architectural shift to functional providers. |
| `dio` | ^5.4.0 | ^5.9.0 | Maintenance update. |
| `just_audio` | ^0.9.36 | ^0.10.x | Stability update. |

### 3.2 State Management Refactoring (Riverpod 3.0)
**Current**: Manual `Provider`, `StreamProvider`, `StateNotifierProvider` definitions.
**Refactoring**:
- **Adopt `riverpod_generator`**: Migrate all providers to `@riverpod` annotated functions/classes.
- **Functional Providers**: Replace `ChangeNotifier` or `StateNotifier` with `Notifier`/`AsyncNotifier`.
- **AutoDispose**: Enable `keepAlive: false` (default) for transient states like `SearchProvider`.

#### 3.2.1 Key Provider Migrations
- `PlayerHandlerProvider` -> `@Riverpod(keepAlive: true)` (Global Singleton)
- `CurrentTrackProvider` -> `@riverpod Stream<MediaItem?>`
- `PlaybackStateProvider` -> `@riverpod Stream<PlaybackState?>`

## 4. Execution Plan
1.  **Phase 1: Backend Upgrade**:
    -   Update `Cargo.toml`.
    -   Fix compilation errors in `main.rs` and `routes/mod.rs`.
    -   Verify API endpoints with `curl` / tests.
2.  **Phase 2: Mobile Upgrade**:
    -   Update `pubspec.yaml` & add `build_runner`.
    -   Run `dart run build_runner build`.
    -   Refactor `player_provider.dart` and `home_provider.dart` iteratively.
3.  **Phase 3: Integration Testing**:
    -   Verify full "Scan -> Play -> Sync" cycle.
