use crate::handlers::{admin, auth, music, playlist};
use crate::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn auth_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/kick/{user_id}", post(auth::kick_user))
        .route("/me", get(auth::get_me).patch(auth::update_profile))
        .route("/avatar", post(auth::upload_avatar))
}

pub fn music_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/scan", post(music::trigger_scan))
        .route("/scan/status", get(music::get_scan_status))
        .route(
            "/playback",
            get(music::get_playback_state).post(music::update_playback_state),
        )
        .route("/stream/{id}", get(music::stream_track))
        .route("/covers/{album_id}", get(music::get_cover))
        .route("/lyrics/{id}", get(music::get_lyrics))
        .route("/artists", get(music::list_artists))
        .route("/albums", get(music::list_albums))
        .route("/tracks", get(music::list_tracks))
        .route("/tracks/{id}", get(music::get_track))
        .route("/search", get(music::global_search))
        .route("/favorites", get(music::list_favorites))
        .route("/favorites/{track_id}", post(music::toggle_favorite))
        .route("/history", get(music::list_history))
        .route("/play/{id}", post(music::record_play))
        .route(
            "/tracks/{track_id}/lyric-offset",
            get(music::get_lyric_offset).post(music::update_lyric_offset),
        )
        .route(
            "/tracks/{track_id}/rescan",
            post(music::rescan_track_metadata),
        )
        .route("/artists/{id}/sync", post(music::sync_artist_metadata))
}

pub fn playlist_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/",
            post(playlist::create_playlist).get(playlist::list_my_playlists),
        )
        .route(
            "/{id}",
            get(playlist::get_playlist)
                .delete(playlist::delete_playlist)
                .patch(playlist::update_playlist),
        )
        .route("/{id}/reorder", post(playlist::reorder_tracks))
        .route(
            "/{id}/tracks/{track_id}",
            post(playlist::add_track).delete(playlist::remove_track),
        )
}

pub fn admin_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/config",
            get(admin::get_admin_config).post(admin::update_admin_config),
        )
        .route("/status", get(admin::get_system_status))
        .route("/sync-artists", post(admin::trigger_artist_sync))
        .route(
            "/sync-artists/missing",
            post(admin::trigger_artist_sync_missing),
        )
        .route(
            "/sync-artists/{id}",
            post(admin::trigger_artist_sync_single),
        )
        .route("/sync-artists/status", get(admin::get_artist_sync_status))
        .route("/artists/{id}/avatar", post(admin::upload_artist_avatar))
        .route("/users", get(admin::list_users))
        .route("/users/{id}/role", post(admin::update_user_role))
        .route("/users/{id}", axum::routing::delete(admin::delete_user))
        .route("/library/organize", post(admin::trigger_library_organize))
}
