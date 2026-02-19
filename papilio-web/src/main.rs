use leptos::wasm_bindgen::JsCast;
use leptos::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use web_sys::Storage;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Track {
    pub id: Uuid,
    pub title: String,
    pub album_id: Option<Uuid>,
    pub artist_id: Option<Uuid>,
    pub duration: i32,
    pub format: Option<String>,
    pub lyrics: Option<String>,
    pub sync_status: SyncStatus,
    #[serde(default)]
    pub is_favorite: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Artist {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Album {
    pub id: Uuid,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GlobalSearchResponse {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<TrackWithFavorite>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TrackWithFavorite {
    #[serde(flatten)]
    pub track: Track,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time: f64,
    pub text: String,
}

#[derive(Clone, Copy)]
struct PlayerContext {
    current_track: RwSignal<Option<Track>>,
    is_playing: RwSignal<bool>,
    is_fullscreen: RwSignal<bool>,
    playlist: RwSignal<Vec<Track>>,
    progress: RwSignal<f64>,
    duration: RwSignal<f64>,
    lyrics: RwSignal<Vec<LyricLine>>,
}

#[derive(Clone, Copy)]
struct AuthContext {
    token: RwSignal<Option<String>>,
}

fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    // 使用 expect 替代 unwrap，虽然 pattern 是常量，但工业级代码需要明确意图
    let time_re = regex::Regex::new(r"\[(\d+):(\d+)[.:](\d+)\]").expect("Invalid regex pattern");
    let clean_re = regex::Regex::new(r"\[[^\]]+\]|<[^>]+>").expect("Invalid regex pattern");

    for line in lrc.lines() {
        // 1. 查找这一行中所有的标准时间戳 [mm:ss.xx]
        let mut timestamps = Vec::new();
        for cap in time_re.captures_iter(line) {
            let min: f64 = cap[1].parse().unwrap_or(0.0);
            let sec: f64 = cap[2].parse().unwrap_or(0.0);
            let ms_str = &cap[3];
            let mut ms: f64 = ms_str.parse().unwrap_or(0.0);
            if ms_str.len() == 2 {
                ms /= 100.0;
            } else if ms_str.len() == 3 {
                ms /= 1000.0;
            }

            timestamps.push(min * 60.0 + sec + ms);
        }

        if !timestamps.is_empty() {
            // 2. 清理掉所有的标签（包括时间戳本身和 AI 逐字标签）
            let text = clean_re.replace_all(line, "").trim().to_string();

            if !text.is_empty() {
                for time in timestamps {
                    lines.push(LyricLine {
                        time,
                        text: text.clone(),
                    });
                }
            }
        }
    }
    // 修复：f64 排序必须处理 NaN 情况，严禁 unwrap
    lines.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    lines
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub is_admin: bool,
}

impl User {
    pub fn display_name(&self) -> String {
        self.nickname
            .clone()
            .unwrap_or_else(|| self.username.clone())
    }

    pub fn avatar_url(&self) -> Option<String> {
        self.avatar
            .as_ref()
            .map(|a| format!("{}/data/avatars/{}", get_api_base_url(), a))
    }
}

async fn fetch_me() -> Result<User, String> {
    api_request("GET", "/api/auth/me", None)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn update_profile_api(
    nickname: Option<String>,
    email: Option<String>,
    password: Option<String>,
) -> Result<User, String> {
    let body = serde_json::json!({
        "nickname": nickname,
        "email": email,
        "password": password,
    });
    api_request("PATCH", "/api/auth/me", Some(body))
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn upload_avatar_api(file: web_sys::File) -> Result<User, String> {
    let storage = window().local_storage().ok().flatten();
    let token = storage.and_then(|s| s.get_item("auth_token").ok().flatten());
    let url = format!("{}/api/auth/avatar", get_api_base_url());

    let form_data = web_sys::FormData::new().map_err(|_| "Failed to create form")?;
    form_data
        .append_with_blob("avatar", &file)
        .map_err(|_| "Failed to append file")?;

    let mut req = gloo_net::http::Request::post(&url);
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {}", t));
    }

    req.body(form_data)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn api_request(
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<gloo_net::http::Response, String> {
    let storage = window().local_storage().ok().flatten();
    let token = storage.and_then(|s: Storage| s.get_item("auth_token").ok().flatten());
    let url = format!("{}{}", get_api_base_url(), path);
    let mut req = match method {
        "POST" => gloo_net::http::Request::post(&url),
        "PATCH" => gloo_net::http::Request::patch(&url),
        _ => gloo_net::http::Request::get(&url),
    };
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {}", t));
    }
    if let Some(b) = body {
        req.json(&b)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())
    } else {
        req.send().await.map_err(|e| e.to_string())
    }
}

fn get_api_base_url() -> String {
    let location = window().location();
    let origin = location
        .origin()
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    // If we are running in development (e.g., trunk serve on 8080),
    // we might still want to point to 3000.
    if origin.contains(":8080") || origin.contains(":8081") {
        "http://localhost:3000".to_string()
    } else {
        origin
    }
}

fn get_cover_url(album_id: Option<Uuid>) -> String {
    match album_id {
        Some(id) => format!("{}/api/music/covers/{}", get_api_base_url(), id),
        None => "".to_string(),
    }
}

async fn fetch_tracks(q: Option<String>) -> Result<Vec<Track>, String> {
    let path = match q {
        Some(query) if !query.is_empty() => format!("/api/music/tracks?q={}", query),
        _ => "/api/music/tracks".to_string(),
    };
    api_request("GET", &path, None)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_global_search(q: String) -> Result<GlobalSearchResponse, String> {
    if q.is_empty() {
        return Ok(GlobalSearchResponse {
            artists: vec![],
            albums: vec![],
            tracks: vec![],
        });
    }
    api_request("GET", &format!("/api/music/search?q={}", q), None)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_favorites() -> Result<Vec<Track>, String> {
    api_request("GET", "/api/music/favorites", None)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn toggle_favorite_api(track_id: Uuid) -> Result<bool, String> {
    let res: serde_json::Value =
        api_request("POST", &format!("/api/music/favorites/{}", track_id), None)
            .await?
            .json()
            .await
            .map_err(|e| e.to_string())?;
    Ok(res["is_favorite"].as_bool().unwrap_or(false))
}

async fn trigger_scan_api() -> Result<(), String> {
    api_request("POST", "/api/music/scan", None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Playlist {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
}

async fn fetch_playlists() -> Result<Vec<Playlist>, String> {
    api_request("GET", "/api/playlists", None)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn create_playlist_api(name: String) -> Result<Playlist, String> {
    let body = serde_json::json!({ "name": name, "is_public": false });
    api_request("POST", "/api/playlists", Some(body))
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_lyrics(id: Uuid) -> Option<String> {
    let resp = api_request("GET", &format!("/api/music/lyrics/{}", id), None)
        .await
        .ok()?;
    if resp.status() == 200 {
        resp.text().await.ok()
    } else {
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ArtistSyncStatus {
    pub is_syncing: bool,
    pub current_count: i32,
    pub total_count: i32,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

async fn fetch_artist_sync_status() -> Result<ArtistSyncStatus, String> {
    api_request("GET", "/api/admin/sync-artists/status", None)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn trigger_artist_sync_api() -> Result<(), String> {
    api_request("POST", "/api/admin/sync-artists", None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

async fn trigger_library_organize_api() -> Result<(), String> {
    api_request("POST", "/api/admin/library/organize", None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[component]
fn App() -> impl IntoView {
    let initial_token = window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item("auth_token").ok().flatten());
    let token = create_rw_signal(initial_token);
    let is_mobile_menu_open = create_rw_signal(false);

    provide_context(AuthContext { token });
    provide_context(PlayerContext {
        current_track: create_rw_signal(None),
        is_playing: create_rw_signal(false),
        is_fullscreen: create_rw_signal(false),
        playlist: create_rw_signal(Vec::new()),
        progress: create_rw_signal(0.0),
        duration: create_rw_signal(0.0),
        lyrics: create_rw_signal(Vec::new()),
    });

    view! {
        <Router>
            <div class="flex flex-col md:flex-row h-screen w-screen bg-papilio-bg text-white overflow-hidden font-sans relative">
                {move || if token.get().is_none() {
                    view! { <AuthScreen /> }.into_view()
                } else {
                    view! {
                        <>
                            // 移动端 Header
                            <header class="md:hidden flex items-center justify-between p-4 bg-papilio-surface border-b border-white/5 z-50">
                                <div class="flex items-center gap-2"><Logo /><span class="font-black italic">"Papilio"</span></div>
                                <button on:click=move |_| is_mobile_menu_open.set(!is_mobile_menu_open.get()) class="text-2xl">"☰"</button>
                            </header>

                            <Sidebar is_open=is_mobile_menu_open />

                            <main class="flex-1 flex flex-col relative overflow-y-auto custom-scrollbar pb-32">
                                <Routes>
                                    <Route path="" view=move || view! { <Home /> }/>
                                    <Route path="/search" view=move || view! { <Search /> }/>
                                    <Route path="/favorites" view=move || view! { <Favorites /> }/>
                                    <Route path="/profile" view=move || view! { <Profile /> }/>
                                    <Route path="/admin" view=move || view! { <Admin /> }/>
                                </Routes>
                            </main>
                            <PlayerBar />
                            <FullscreenPlayer />
                        </>
                    }.into_view()
                }}
            </div>
        </Router>
    }
}

#[component]
fn Sidebar(is_open: RwSignal<bool>) -> impl IntoView {
    let auth = use_context::<AuthContext>().expect("auth");
    let playlists_res = create_resource(|| (), |_| async move { fetch_playlists().await });
    let user_res = create_resource(|| (), |_| async move { fetch_me().await });

    let logout = move |_| {
        let _ = window()
            .local_storage()
            .ok()
            .flatten()
            .and_then(|s| s.remove_item("auth_token").ok());
        auth.token.set(None);
    };

    let add_playlist = move |_| {
        spawn_local(async move {
            if let Ok(_) = create_playlist_api("New Playlist".to_string()).await {
                playlists_res.refetch();
            }
        });
    };

    view! {
        <aside
            class="fixed md:relative inset-y-0 left-0 w-64 bg-papilio-surface md:bg-white/5 backdrop-blur-3xl border-r border-white/10 flex flex-col p-6 gap-8 z-[70] transition-transform duration-300 md:translate-x-0"
            class:translate-x-0=move || is_open.get()
            class=move || if !is_open.get() { "-translate-x-full" } else { "" }
        >
            <div class="flex items-center justify-between">
                <div class="flex items-center gap-3"><Logo /><h1 class="text-xl font-bold tracking-tight italic">"Papilio"</h1></div>
                <button class="md:hidden text-2xl" on:click=move |_| is_open.set(false)>"✕"</button>
            </div>

            <nav class="flex flex-col gap-2 flex-1" on:click=move |_| is_open.set(false)>
                <A href="/" active_class="bg-papilio-accent text-white" class="flex items-center gap-3 p-3 rounded-xl transition-all hover:bg-white/5" exact=true>"🏠 首页"</A>
                <A href="/search" active_class="bg-papilio-accent text-white" class="flex items-center gap-3 p-3 rounded-xl transition-all hover:bg-white/5">"🔍 搜索"</A>
                <A href="/favorites" active_class="bg-papilio-accent text-white" class="flex items-center gap-3 p-3 rounded-xl transition-all hover:bg-white/5">"❤️ 收藏"</A>
                <A href="/profile" active_class="bg-papilio-accent text-white" class="flex items-center gap-3 p-3 rounded-xl transition-all hover:bg-white/5">"👤 个人中心"</A>

                {move || user_res.get().map(|res| if let Ok(user) = res {
                    if user.is_admin {
                        view! {
                            <A href="/admin" active_class="bg-papilio-cyan/20 text-papilio-cyan" class="flex items-center gap-3 p-3 rounded-xl transition-all hover:bg-white/5 border border-papilio-cyan/20 mt-2">
                                "⚙️ 管理员后台"
                            </A>
                        }.into_view()
                    } else { view! {}.into_view() }
                } else { view! {}.into_view() })}

                <div class="h-px bg-white/5 my-4"></div>
                <div class="text-[10px] text-papilio-muted uppercase tracking-widest px-3 mb-2">"My Playlists"</div>

                <Suspense fallback=move || view! { <div class="px-3 text-xs text-white/20">"Loading..."</div> }>
                    {move || playlists_res.get().map(|res| match res {
                        Ok(list) => list.into_iter().map(|p| view! {
                            <A href=format!("/playlist/{}", p.id) class="flex items-center gap-3 p-3 rounded-xl hover:bg-white/5 text-sm truncate opacity-70 hover:opacity-100 transition-all">
                                "💿" {p.name}
                            </A>
                        }).collect_view(),
                        Err(_) => view! { <div class="px-3 text-xs text-red-400">"Failed to load"</div> }.into_view()
                    })}
                </Suspense>

                <button class="flex items-center gap-3 p-3 rounded-xl hover:bg-white/5 text-papilio-muted text-sm italic w-full text-left" on:click=add_playlist>"+ 新建列表"</button>

                <button
                    class="flex items-center gap-3 p-3 rounded-xl hover:bg-papilio-cyan/20 text-papilio-cyan text-sm transition-all mt-4 border border-papilio-cyan/20"
                    on:click=move |_| { spawn_local(async { let _ = trigger_scan_api().await; }); }
                >
                    "🔄 扫描媒体库"
                </button>
            </nav>

            <button class="text-left text-xs text-papilio-muted hover:text-red-400 p-2 transition-colors" on:click=logout>"登出账号"</button>
        </aside>
    }
}

#[component]
fn Home() -> impl IntoView {
    let tracks_res = create_resource(|| (), |_| async move { fetch_tracks(None).await });
    view! {
        <div class="p-6 md:p-10 flex flex-col gap-10">
            <section class="h-60 md:h-80 rounded-[2.5rem] bg-gradient-to-br from-papilio-accent/40 via-papilio-surface to-papilio-cyan/20 border border-white/10 p-8 md:p-12 flex flex-col justify-end relative overflow-hidden shadow-2xl shrink-0">
                <h2 class="text-4xl md:text-7xl font-black tracking-tighter mb-2 leading-none">"蝶变音律"</h2>
                <p class="text-white/70 text-base md:text-xl font-light">"私人高保真资源已同步。"</p>
            </section>
            <div class="flex-1">
                <h3 class="text-2xl font-bold mb-8 flex items-center gap-3"><span class="w-1.5 h-6 bg-papilio-cyan rounded-full"></span>"推荐曲目"</h3>
                <Suspense fallback=move || view! { <div class="text-papilio-muted">"Loading..."</div> }>
                    {move || tracks_res.get().map(|res| match res {
                        Ok(data) => view! {
                            <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6 md:gap-8">
                                {data.clone().into_iter().map(|track| {
                                    let full_list = data.clone();
                                    view! { <TrackCard track=track playlist=full_list /> }
                                }).collect_view()}
                            </div>
                        }.into_view(),
                        Err(_) => view! { <p class="text-red-400 text-center py-20">"权限验证失败，请重新登录"</p> }.into_view()
                    })}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn Favorites() -> impl IntoView {
    let tracks_res = create_resource(|| (), |_| async move { fetch_favorites().await });
    view! {
        <div class="p-6 md:p-10 flex flex-col gap-10">
            <h2 class="text-4xl md:text-6xl font-black tracking-tighter">"我的收藏"</h2>
            <div class="flex-1">
                <Suspense fallback=move || view! { <div class="text-papilio-muted text-center py-20 animate-pulse">"加载中..."</div> }>
                    {move || tracks_res.get().map(|res| match res {
                        Ok(data) => if data.is_empty() {
                            view! { <div class="text-center py-20 text-papilio-muted text-xl border border-dashed border-white/10 rounded-3xl">"暂无收藏的音律"</div> }.into_view()
                        } else {
                            view! {
                                <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6 md:gap-8">
                                    {data.clone().into_iter().map(|track| {
                                        let full_list = data.clone();
                                        view! { <TrackCard track=track playlist=full_list /> }
                                    }).collect_view()}
                                </div>
                            }.into_view()
                        },
                        Err(_) => view! { <p class="text-red-400 text-center py-20">"获取收藏失败"</p> }.into_view()
                    })}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn Search() -> impl IntoView {
    let (query, set_query) = create_signal(String::new());
    let search_res = create_resource(
        move || query.get(),
        |q| async move { fetch_global_search(q).await },
    );

    view! {
        <div class="p-6 md:p-10 flex flex-col gap-10">
            <div class="relative w-full max-w-2xl mx-auto md:mx-0">
                <input type="text" placeholder="搜索曲目、艺人或专辑..." class="w-full bg-white/5 border border-white/10 rounded-2xl py-4 md:py-5 px-12 text-lg md:text-2xl focus:outline-none focus:border-papilio-accent transition-all backdrop-blur-md shadow-2xl" on:input=move |ev| { set_query.set(event_target_value(&ev)); } prop:value=query />
                <span class="absolute left-4 top-1/2 -translate-y-1/2 text-2xl opacity-40">"🔍"</span>
            </div>

            <div class="flex-1 flex flex-col gap-12">
                <Suspense fallback=move || view! { <div class="text-papilio-muted text-center py-20 animate-pulse">"正在搜寻..."</div> }>
                    {move || search_res.get().map(|res| match res {
                        Ok(data) => {
                            if data.tracks.is_empty() && data.artists.is_empty() && data.albums.is_empty() {
                                view! { <div class="text-center py-20 text-papilio-muted text-xl border border-dashed border-white/10 rounded-3xl">"未找到匹配的结果"</div> }.into_view()
                            } else {
                                view! {
                                    <div class="flex flex-col gap-12">
                                        {if !data.artists.is_empty() {
                                            view! {
                                                <section>
                                                    <h3 class="text-xl font-bold mb-4 opacity-60 uppercase tracking-widest text-papilio-cyan">"匹配到的艺人"</h3>
                                                    <div class="flex flex-wrap gap-4">
                                                        {data.artists.into_iter().map(|artist| view! {
                                                            <div class="bg-white/5 border border-white/10 px-6 py-3 rounded-2xl hover:bg-papilio-accent/20 transition-all cursor-pointer group">
                                                                <span class="text-white/60 group-hover:text-white transition-colors">{artist.name}</span>
                                                            </div>
                                                        }).collect_view()}
                                                    </div>
                                                </section>
                                            }.into_view()
                                        } else { view! {}.into_view() }}

                                        {if !data.tracks.is_empty() {
                                            let tracks_only: Vec<Track> = data.tracks.iter().map(|t| t.track.clone()).collect();
                                            view! {
                                                <section>
                                                    <h3 class="text-xl font-bold mb-6 opacity-60 uppercase tracking-widest text-papilio-cyan">"匹配到的单曲"</h3>
                                                    <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-6 md:gap-8">
                                                        {data.tracks.into_iter().map(|t| {
                                                            let full_list = tracks_only.clone();
                                                            view! { <TrackCard track=t.track playlist=full_list /> }
                                                        }).collect_view()}
                                                    </div>
                                                </section>
                                            }.into_view()
                                        } else { view! {}.into_view() }}
                                    </div>
                                }.into_view()
                            }
                        },
                        Err(_) => view! { <p class="text-red-400 text-center py-20">"搜索请求失败"</p> }.into_view()
                    })}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn TrackCard(track: Track, playlist: Vec<Track>) -> impl IntoView {
    let player = use_context::<PlayerContext>().expect("context not found");
    let (is_fav, set_is_fav) = create_signal(track.is_favorite);
    let cover_url = get_cover_url(track.album_id);
    let on_click = {
        let track = track.clone();
        let playlist = playlist.clone();
        move |_| {
            player.playlist.set(playlist.clone());
            player.current_track.set(Some(track.clone()));
            player.is_playing.set(true);
        }
    };
    let toggle_fav = {
        let track_id = track.id;
        move |ev: web_sys::MouseEvent| {
            ev.stop_propagation();
            spawn_local(async move {
                if let Ok(new_status) = toggle_favorite_api(track_id).await {
                    set_is_fav.set(new_status);
                }
            });
        }
    };
    view! {
        <div class="group cursor-pointer relative" on:click=on_click>
            <div class="aspect-square rounded-[2rem] overflow-hidden relative border border-white/10 shadow-xl transition-all duration-500 hover:scale-[1.02] active:scale-[0.98] group-hover:shadow-[0_20px_40px_rgba(0,0,0,0.4)]">
                <img src=cover_url class="w-full h-full object-cover transition-all duration-700 group-hover:scale-110" />
                <button class="absolute top-4 right-4 w-10 h-10 rounded-full bg-black/40 backdrop-blur-md flex items-center justify-center transition-all opacity-0 group-hover:opacity-100 hover:scale-110 active:scale-90 z-10" on:click=toggle_fav>{move || if is_fav.get() { "❤️" } else { "🤍" }}</button>
                <div class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-all duration-500 flex items-center justify-center pointer-events-none">
                    <div class="w-16 h-16 rounded-full bg-white/10 backdrop-blur-md border border-white/20 flex items-center justify-center text-white text-3xl">"▶"</div>
                </div>
            </div>
            <div class="mt-4 px-2">
                <div class="font-bold text-white/90 text-sm md:text-base group-hover:text-papilio-cyan transition-colors truncate">{track.title}</div>
                <div class="text-[9px] md:text-[10px] text-papilio-muted mt-1 uppercase tracking-widest opacity-60 font-mono truncate">{format!(".{}", track.format.unwrap_or_default())}</div>
            </div>
        </div>
    }
}

#[component]
fn PlayerBar() -> impl IntoView {
    let player = use_context::<PlayerContext>().expect("context not found");
    let audio_ref = create_node_ref::<leptos::html::Audio>();
    create_effect(move |_| {
        if let Some(track) = player.current_track.get() {
            if let Some(audio) = audio_ref.get() {
                audio.set_src(&format!(
                    "{}/api/music/stream/{}",
                    get_api_base_url(),
                    track.id
                ));
                let _ = audio.play();

                // 使用歌词服务
                spawn_local(async move {
                    if let Some(lrc_text) = fetch_lyrics(track.id).await {
                        player.lyrics.set(parse_lrc(&lrc_text));
                    } else {
                        player.lyrics.set(Vec::new());
                    }
                });
            }
        }
    });
    let next_track = move || {
        let current = player.current_track.get();
        let list = player.playlist.get();
        if let Some(curr) = current {
            if let Some(pos) = list.iter().position(|t| t.id == curr.id) {
                let next_idx = (pos + 1) % list.len();
                player.current_track.set(Some(list[next_idx].clone()));
                player.is_playing.set(true);
            }
        }
    };
    let toggle_play = move |_| {
        if let Some(audio) = audio_ref.get() {
            if player.is_playing.get() {
                let _ = audio.pause();
                player.is_playing.set(false);
            } else {
                let _ = audio.play();
                player.is_playing.set(true);
            }
        }
    };
    view! {
        <footer class="fixed bottom-0 left-0 right-0 h-24 bg-papilio-surface/80 backdrop-blur-[40px] border-t border-white/5 px-4 md:px-8 flex items-center justify-between z-[60] shadow-2xl">
            <audio node_ref=audio_ref on:timeupdate=move |_| if let Some(a) = audio_ref.get() { player.progress.set(a.current_time()); player.duration.set(a.duration()); } on:ended=move |_| next_track() />
            <div class="flex items-center gap-3 md:gap-5 w-1/4">
                {move || player.current_track.get().map(|track| {
                    let cover_url = get_cover_url(track.album_id);
                    view! {
                        <>
                            <img src=cover_url class="w-12 h-12 md:w-16 md:h-16 rounded-xl object-cover border border-white/10" />
                            <div class="overflow-hidden"><div class="font-bold truncate text-sm md:text-lg">{track.title}</div><button class="text-[10px] text-papilio-cyan hover:underline uppercase tracking-tighter" on:click=move |_| player.is_fullscreen.set(true)>"Show Lyrics"</button></div>
                        </>
                    }
                })}
            </div>
            <div class="flex flex-col items-center gap-2 md:gap-3 flex-1 md:w-2/4">
                <div class="flex items-center gap-6 md:gap-10">
                    <button class="text-xl md:text-2xl text-white/40 hover:text-white transition-colors">"⏮"</button>
                    <button class="w-10 h-10 md:w-14 md:h-14 rounded-full bg-white text-black flex items-center justify-center text-xl md:text-3xl shadow-xl hover:scale-105 active:scale-95 transition-all" on:click=toggle_play>{move || if player.is_playing.get() { "⏸" } else { "▶" }}</button>
                    <button class="text-xl md:text-2xl text-white/40 hover:text-white transition-colors" on:click=move |_| next_track()>"⏭"</button>
                </div>
                <div class="w-full max-w-2xl flex items-center gap-3 text-[9px] font-mono text-papilio-muted">
                    <div class="flex-1 h-1 bg-white/5 rounded-full overflow-hidden relative">
                        <div class="absolute top-0 left-0 h-full bg-papilio-accent shadow-[0_0_10px_#8B5CF6]" style:width=move || format!("{}%", (player.progress.get() / player.duration.get().max(1.0)) * 100.0)></div>
                    </div>
                </div>
            </div>
            <div class="flex items-center justify-end w-1/4">
                <button class="text-2xl opacity-60 hover:opacity-100 hover:scale-110 transition-all" on:click=move |_| player.is_fullscreen.set(!player.is_fullscreen.get())>"⛶"</button>
            </div>
        </footer>
    }
}

#[component]
fn FullscreenPlayer() -> impl IntoView {
    let player = use_context::<PlayerContext>().expect("context not found");
    let active_index = move || {
        let current_time = player.progress.get();
        let list = player.lyrics.get();
        list.iter()
            .rposition(|line| line.time <= current_time)
            .unwrap_or(0)
    };
    view! {
        <div class="fixed inset-0 z-[100] bg-papilio-bg transition-all duration-700 ease-[cubic-bezier(0.85,0,0.15,1)] flex flex-col" class:translate-y-full=move || !player.is_fullscreen.get() class:translate-y-0=move || player.is_fullscreen.get()>
            {move || player.current_track.get().map(|track| {
                let cover_url = get_cover_url(track.album_id);
                view! { <div class="absolute inset-0 z-0"><img src=cover_url class="w-full h-full object-cover blur-[100px] opacity-40 scale-125" /><div class="absolute inset-0 bg-gradient-to-b from-black/40 via-papilio-bg/90 to-papilio-bg"></div></div> }
            })}
            <header class="p-6 md:p-10 flex justify-between items-center z-10"><button class="w-12 h-12 rounded-full bg-white/5 hover:bg-white/10 flex items-center justify-center text-3xl" on:click=move |_| player.is_fullscreen.set(false)>"↓"</button><div class="text-center"><div class="text-[10px] uppercase tracking-[0.4em] text-papilio-cyan font-bold opacity-80">"Immersion Mode"</div><div class="text-lg md:text-3xl font-black mt-2 tracking-tight">{move || player.current_track.get().map(|t| t.title).unwrap_or_default()}</div></div><div class="w-12"></div></header>
            <div class="flex-1 flex flex-col md:flex-row items-center justify-center gap-10 md:gap-32 p-6 md:p-20 z-10 overflow-hidden">
                <div class="w-full max-w-[300px] md:max-w-[500px] aspect-square rounded-[3rem] md:rounded-[4rem] overflow-hidden shadow-[0_50px_100px_rgba(0,0,0,0.8)] border border-white/10">
                    {move || player.current_track.get().map(|track| { let cover_url = get_cover_url(track.album_id); view! { <img src=cover_url class="w-full h-full object-cover" /> } })}
                </div>
                <div class="flex-1 w-full max-w-3xl h-[400px] md:h-full flex flex-col justify-center relative overflow-hidden text-center md:text-left">
                    <div class="transition-all duration-700 ease-out" style:transform=move || format!("translateY(-{}px)", active_index() as f64 * (if window().inner_width().unwrap_or_default().as_f64().unwrap_or(0.0) < 768.0 { 60.0 } else { 90.0 }))>
                        {move || player.lyrics.get().into_iter().enumerate().map(|(i, line)| {
                            let is_active = i == active_index();
                            view! { <div
                                class="h-[60px] md:h-[90px] flex items-center text-2xl md:text-5xl font-black transition-all duration-700"
                                class:text-white=is_active
                                class:opacity-100=is_active
                                class:scale-105=is_active
                                class:blur-sm=!is_active
                                class=move || if !is_active { "text-white/10" } else { "" }
                            >{line.text}</div> }
                        }).collect_view()}
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn Profile() -> impl IntoView {
    let user_res = create_resource(|| (), |_| async move { fetch_me().await });

    let (nickname, set_nickname) = create_signal(String::new());
    let (email, set_email) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (message, set_message) = create_signal(Option::<(String, bool)>::None);

    create_effect(move |_| {
        if let Some(Ok(user)) = user_res.get() {
            set_nickname.set(user.nickname.unwrap_or_default());
            set_email.set(user.email.unwrap_or_default());
        }
    });

    let save_profile = move |_| {
        let nick = if nickname.get().is_empty() {
            None
        } else {
            Some(nickname.get())
        };
        let mail = if email.get().is_empty() {
            None
        } else {
            Some(email.get())
        };
        let pass = if password.get().is_empty() {
            None
        } else {
            Some(password.get())
        };

        spawn_local(async move {
            match update_profile_api(nick, mail, pass).await {
                Ok(_) => {
                    set_message.set(Some(("资料更新成功".to_string(), true)));
                    user_res.refetch();
                    set_password.set(String::new());
                }
                Err(e) => set_message.set(Some((format!("更新失败: {}", e), false))),
            }
        });
    };

    view! {
        <div class="p-6 md:p-10 max-w-4xl">
            <h2 class="text-4xl md:text-6xl font-black tracking-tighter mb-10">"个人资料"</h2>

            <Suspense fallback=move || view! { <div class="animate-pulse">"加载中..."</div> }>
                {move || user_res.get().map(|res| match res {
                    Ok(user) => view! {
                        <div class="flex flex-col gap-8 bg-white/5 border border-white/10 rounded-[2.5rem] p-8 md:p-12 backdrop-blur-xl shadow-2xl">
                            {move || message.get().map(|(msg, success)| view! {
                                <div class=format!("p-4 rounded-2xl text-center font-bold text-sm mb-4 {}", if success { "bg-green-500/20 text-green-400" } else { "bg-red-500/20 text-red-400" })>
                                    {msg}
                                </div>
                            })}

                            <div class="flex items-center gap-6 mb-4">
                                <div
                                    class="w-20 h-20 rounded-3xl bg-papilio-accent/30 flex items-center justify-center text-3xl font-black border border-papilio-accent/20 cursor-pointer overflow-hidden group relative"
                                    on:click=move |_| {
                                        if let Some(el) = window().document().and_then(|d| d.get_element_by_id("avatar-upload")) {
                                            let input: web_sys::HtmlInputElement = el.unchecked_into();
                                            input.click();
                                        }
                                    }
                                >
                                    {
                                        let user_avatar = user.clone();
                                        move || if let Some(url) = user_avatar.avatar_url() {
                                            view! { <img src=url class="w-full h-full object-cover group-hover:opacity-50 transition-opacity" /> }.into_view()
                                        } else {
                                            view! { <span>{user_avatar.display_name().chars().next().unwrap_or('?').to_uppercase().to_string()}</span> }.into_view()
                                        }
                                    }
                                    <div class="absolute inset-0 items-center justify-center hidden group-hover:flex bg-black/20 text-xs">"更换"</div>
                                    <input
                                        type="file"
                                        id="avatar-upload"
                                        class="hidden"
                                        accept="image/*"
                                        on:change=move |ev| {
                                            let target = event_target::<web_sys::HtmlInputElement>(&ev);
                                            if let Some(files) = target.files() {
                                                if let Some(file) = files.get(0) {
                                                    spawn_local(async move {
                                                        if let Ok(_) = upload_avatar_api(file).await {
                                                            user_res.refetch();
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    />
                                </div>
                                <div>
                                    <div class="text-2xl font-bold">{user.display_name()}</div>
                                    <div class="text-papilio-muted text-sm tracking-widest uppercase opacity-60 font-mono">"@" {user.username.clone()}</div>
                                </div>
                            </div>

                            <div class="grid md:grid-cols-2 gap-8">
                                <div class="flex flex-col gap-2">
                                    <label class="text-[10px] uppercase tracking-widest text-papilio-muted px-2">"昵称 (Nickname)"</label>
                                    <input type="text" prop:value=nickname on:input=move |ev| set_nickname.set(event_target_value(&ev)) class="bg-black/20 border border-white/5 rounded-2xl p-4 focus:outline-none focus:border-papilio-cyan transition-all" />
                                </div>
                                <div class="flex flex-col gap-2">
                                    <label class="text-[10px] uppercase tracking-widest text-papilio-muted px-2">"邮箱 (Email)"</label>
                                    <input type="email" prop:value=email on:input=move |ev| set_email.set(event_target_value(&ev)) class="bg-black/20 border border-white/5 rounded-2xl p-4 focus:outline-none focus:border-papilio-cyan transition-all" />
                                </div>
                                <div class="flex flex-col gap-2 md:col-span-2">
                                    <label class="text-[10px] uppercase tracking-widest text-papilio-muted px-2">"修改密码 (留空表示不修改)"</label>
                                    <input type="password" prop:value=password on:input=move |ev| set_password.set(event_target_value(&ev)) placeholder="••••••••" class="bg-black/20 border border-white/5 rounded-2xl p-4 focus:outline-none focus:border-papilio-cyan transition-all" />
                                </div>
                            </div>

                            <button on:click=save_profile class="mt-4 bg-white text-black font-black py-5 rounded-2xl hover:scale-[1.02] active:scale-95 transition-all shadow-xl shadow-white/5">
                                "保存所有更改"
                            </button>
                        </div>
                    }.into_view(),
                    Err(_) => view! { <div class="text-red-400">"获取用户信息失败"</div> }.into_view()
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn AuthScreen() -> impl IntoView {
    let auth = use_context::<AuthContext>().expect("auth");
    let (is_register, set_is_register) = create_signal(false);
    let (username, set_username) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (error, set_error) = create_signal(Option::<String>::None);
    let handle_auth = move |_| {
        let u = username.get();
        let p = password.get();
        let reg = is_register.get();
        spawn_local(async move {
            let path = if reg {
                "/api/auth/register"
            } else {
                "/api/auth/login"
            };
            let body = serde_json::json!({ "username": u, "password": p });
            if let Ok(resp) = api_request("POST", path, Some(body)).await {
                if resp.status() == 200 || resp.status() == 201 {
                    let data: serde_json::Value = resp.json().await.unwrap_or_default();
                    if let Some(t) = data["token"].as_str() {
                        let _ = window()
                            .local_storage()
                            .ok()
                            .flatten()
                            .and_then(|s| s.set_item("auth_token", t).ok());
                        auth.token.set(Some(t.to_string()));
                    } else if reg {
                        set_is_register.set(false);
                        set_error.set(Some("注册成功，请登录".to_string()));
                    }
                } else {
                    set_error.set(Some("账号或密码错误".to_string()));
                }
            } else {
                set_error.set(Some("网络连接失败".to_string()));
            }
        });
    };
    view! {
        <div class="fixed inset-0 z-[200] flex items-center justify-center bg-papilio-bg">
            <div class="w-full h-full md:w-[450px] md:h-auto bg-papilio-surface md:bg-white/5 md:border border-white/10 p-10 md:rounded-[3rem] shadow-2xl flex flex-col justify-center md:justify-start gap-10 relative overflow-hidden">
                <div class="absolute -top-40 -left-40 w-80 h-80 bg-papilio-accent/30 blur-[100px] rounded-full"></div>
                <div class="flex flex-col items-center gap-6 text-center"><Logo /><h2 class="text-5xl font-black italic tracking-tighter">"Papilio"</h2><p class="text-papilio-muted text-xs tracking-[0.3em] uppercase font-bold">{move || if is_register.get() { "Create Account" } else { "Welcome Back" }}</p></div>
                <div class="flex flex-col gap-5">
                    {move || error.get().map(|e| view! { <div class="bg-red-500/20 border border-red-500/20 text-red-400 p-4 rounded-2xl text-xs text-center font-bold tracking-wide">{e}</div> })}
                    <input type="text" placeholder="Username" class="bg-white/5 border border-white/10 rounded-2xl px-6 py-4 text-lg focus:outline-none focus:border-papilio-cyan transition-all placeholder:text-white/20" on:input=move |ev| set_username.set(event_target_value(&ev)) />
                    <input type="password" placeholder="Password" class="bg-white/5 border border-white/10 rounded-2xl px-6 py-4 text-lg focus:outline-none focus:border-papilio-cyan transition-all placeholder:text-white/20" on:input=move |ev| set_password.set(event_target_value(&ev)) />
                    <button class="bg-white text-black font-black py-4 rounded-2xl shadow-[0_10px_30px_rgba(255,255,255,0.2)] hover:scale-[1.02] active:scale-95 transition-all mt-4 text-lg" on:click=handle_auth>{move || if is_register.get() { "SIGN UP" } else { "SIGN IN" }}</button>
                </div>
                <button class="text-papilio-muted text-sm hover:text-white transition-colors font-medium tracking-wide" on:click=move |_| set_is_register.set(!is_register.get())>{move || if is_register.get() { "Already have an account? Login" } else { "Don't have an account? Register" }}</button>
            </div>
        </div>
    }
}

#[component]
fn Admin() -> impl IntoView {
    // 1. 定义触发同步的 Action
    let sync_action = create_action(move |_: &()| async move { trigger_artist_sync_api().await });

    let organize_action =
        create_action(move |_: &()| async move { trigger_library_organize_api().await });

    // 2. 定义状态轮询：通过一个自增的时间信号驱动 Resource
    let poll_tick = create_rw_signal(0);

    // 使用 gloo_timers 定时器，并在销毁时由 handle 自动清理
    let interval_handle = gloo_timers::callback::Interval::new(2000, move || {
        poll_tick.update(|n| *n += 1);
    });

    on_cleanup(move || {
        drop(interval_handle);
    });

    // Resource 会在 poll_tick 变化时自动刷新
    let sync_status_res = create_resource(
        move || poll_tick.get(),
        |_| async move { fetch_artist_sync_status().await },
    );

    let start_sync = move |_| {
        sync_action.dispatch(());
    };

    view! {
        <div class="p-6 md:p-10 max-w-4xl">
            <h2 class="text-4xl md:text-6xl font-black tracking-tighter mb-10">"管理员控制台"</h2>

            <div class="grid gap-8">
                <section class="bg-white/5 border border-white/10 rounded-[2.5rem] p-8 md:p-12 backdrop-blur-xl shadow-2xl">
                    <h3 class="text-2xl font-bold mb-6 flex items-center gap-3">
                        <span class="w-1.5 h-6 bg-papilio-cyan rounded-full"></span>"元数据治理"
                    </h3>

                    <div class="flex flex-col gap-6">
                        <div class="flex items-center justify-between bg-black/20 p-6 rounded-3xl border border-white/5">
                            <div>
                                <div class="font-bold text-lg">"同步歌手信息"</div>
                                <div class="text-sm text-papilio-muted">"从 MusicBrainz 补全缺失的歌手元数据"</div>
                            </div>
                            <button
                                on:click=start_sync
                                disabled=move || {
                                    let is_pending = sync_action.pending().get();
                                    let is_syncing = sync_status_res.get().and_then(|r| r.ok()).map(|s| s.is_syncing).unwrap_or(false);
                                    is_pending || is_syncing
                                }
                                class="bg-papilio-cyan text-black font-bold px-8 py-3 rounded-2xl hover:scale-105 active:scale-95 transition-all disabled:opacity-50 disabled:grayscale"
                            >
                                {move || {
                                    if sync_action.pending().get() { "请求中..." }
                                    else if sync_status_res.get().and_then(|r| r.ok()).map(|s| s.is_syncing).unwrap_or(false) { "正在同步..." }
                                    else { "开始批量同步" }
                                }}
                            </button>
                        </div>

                        <div class="flex items-center justify-between bg-black/20 p-6 rounded-3xl border border-white/5">
                            <div>
                                <div class="font-bold text-lg">"物理文件整理"</div>
                                <div class="text-sm text-papilio-muted">"按 {歌手}/{专辑}/{标题} 自动归类物理文件并同步资产"</div>
                            </div>
                            <button
                                on:click=move |_| organize_action.dispatch(())
                                disabled=move || organize_action.pending().get()
                                class="bg-white/10 text-white font-bold px-8 py-3 rounded-2xl hover:bg-white/20 active:scale-95 transition-all disabled:opacity-50"
                            >
                                {move || {
                                    if organize_action.pending().get() { "整理中..." }
                                    else { "立即整理" }
                                }}
                            </button>
                        </div>

                        <Suspense fallback=move || view! { <div class="animate-pulse h-20 bg-white/5 rounded-3xl"></div> }>
                            {move || sync_status_res.get().map(|res| match res {
                                Ok(status) if status.is_syncing || status.current_count > 0 => {
                                    let progress = (status.current_count as f32 / status.total_count.max(1) as f32) * 100.0;
                                    view! {
                                        <div class="bg-black/20 p-6 rounded-3xl border border-white/5 flex flex-col gap-4">
                                            <div class="flex justify-between text-sm font-mono">
                                                <span class="text-papilio-cyan">"同步进度: " {status.current_count} " / " {status.total_count}</span>
                                                <span>{format!("{:.1}%", progress)}</span>
                                            </div>
                                            <div class="w-full h-2 bg-white/5 rounded-full overflow-hidden">
                                                <div
                                                    class="h-full bg-papilio-cyan transition-all duration-500 shadow-[0_0_10px_#22D3EE]"
                                                    style:width=format!("{}%", progress)
                                                ></div>
                                            </div>
                                            {status.last_error.map(|err| view! {
                                                <div class="text-xs text-red-400 mt-2 bg-red-400/10 p-3 rounded-xl border border-red-400/20">
                                                    "最后一次错误: " {err}
                                                </div>
                                            })}
                                        </div>
                                    }.into_view()
                                },
                                _ => view! {}.into_view()
                            })}
                        </Suspense>
                    </div>
                </section>

                <section class="bg-white/5 border border-white/10 rounded-[2.5rem] p-8 md:p-12 backdrop-blur-xl opacity-50 grayscale pointer-events-none">
                    <h3 class="text-2xl font-bold mb-6 flex items-center gap-3">
                        <span class="w-1.5 h-6 bg-papilio-muted rounded-full"></span>"系统监控 (开发中)"
                    </h3>
                </section>
            </div>
        </div>
    }
}

#[component]
fn Logo() -> impl IntoView {
    view! {
        <img
            src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAABAAAAAQACAIAAADwf7zUAADX/GNhQlgAANf8anVtYgAAAB5qdW1kYzJwYQARABCAAACqADibcQNjMnBhAAAANvRqdW1iAAAAR2p1bWRjMm1hABEAEIAAAKoAOJtxA3VybjpjMnBhOjI3Yzc3NzBmLTk5Y2EtNDM3Zi1iOGU4LTc4ZTJiMmY0ZWU4NwAAAAHBanVtYgAAAClqdW1kYzJhcwARABCAAACqADibcQNjMnBhLmFzc2VydGlvbnMAAAAA5Wp1bWIAAAApanVtZGNib3IAEQAQgAAAqgA4m3EDYzJwYS5hY3Rpb25zLnYyAAAAALRjYm9yoWdhY3Rpb25zgqNmYWN0aW9ubGMycGEuY3JlYXRlZG1zb2Z0d2FyZUFnZW50v2RuYW1lZkdQVC00b/9xZGlnaXRhbFNvdXJjZVR5cGV4Rmh0dHA6Ly9jdi5pcHRjLm9yZy9uZXdzY29kZXMvZGlnaXRhbHNvdXJjZXR5cGUvdHJhaW5lZEFsZ29yaXRobWljTWVkaWGhZmFjdGlvbm5jMnBhLmNvbnZlcnRlZAAAAKtqdW1iAAAAKGp1bWRjYm9yABEAEIAAAKoAOJtxA2MycGEuaGFzaC5kYXRhAAAAAHtjYm9ypWpleGNsdXNpb25zgaJlc3RhcnQYIWZsZW5ndGgZNyZkbmFtZW5qdW1iZiBtYW5pZmVzdGNhbGdmc2hhMjU2ZGhhc2hYIMz1pU5lOQCirtcXVrdPiVXzeY4xEqW0mRYYahSMQ3enY3BhZEgAAAAAAAAAAAAAAe1qdW1iAAAAJ2p1bWRjMmNsABEAEIAAAKoAOJtxA2MycGEuY2xhaW0udjIAAAABvmNib3Kmamluc3RhbmNlSUR4LHhtcDppaWQ6YTZmZjA2ZTQtYWI5Zi00NjJiLTkxZTUtYzU0Mzg4MGRiMWNidGNsYWltX2dlbmVyYXRvcl9pbmZvv2RuYW1lZ0NoYXRHUFR3b3JnLmNvbnRlbnRhdXRoLmMycGFfcnNlMC4wLjD/aXNpZ25hdHVyZXhNc2VsZiNqdW1iZj0vYzJwYS91cm46YzJwYToyN2M3NzcwZi05OWNhLTQzN2YtYjhlOC03OGUyYjJmNGVlODcvYzJwYS5zaWduYXR1cmVyY3JlYXRlZF9hc3NlcnRpb25zgqJjdXJs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AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA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 Logo"
            alt="Papilio Logo"
            class="w-12 h-12 md:w-12 md:h-12"
        />
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}
