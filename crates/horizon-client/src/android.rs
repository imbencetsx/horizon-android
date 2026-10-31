//! Android's NativeActivity entry point. Game logic stays in the shared client.
use std::{path::PathBuf, sync::OnceLock};
use winit::platform::android::activity::AndroidApp;

pub(crate) static SUSPENDED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static APP: OnceLock<AndroidApp> = OnceLock::new();
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static RESOURCES: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn app() -> AndroidApp {
    APP.get()
        .expect("Android activity initialized before event loop")
        .clone()
}

pub(crate) fn arguments() -> Vec<String> {
    let root = ROOT.get().expect("Android data directory initialized");
    let mut args = vec![
        "--resources".into(),
        RESOURCES
            .get()
            .expect("Android resource directory initialized")
            .to_string_lossy()
            .into_owned(),
        "--game-dir".into(),
        root.join("game").to_string_lossy().into_owned(),
    ];
    if option_env!("HORIZON_ANDROID_AGENT_SANDBOX") == Some("1") {
        args.extend([
            "--control-socket".to_owned(),
            root.join("game/control.json")
                .to_string_lossy()
                .into_owned(),
        ]);
    }
    if option_env!("HORIZON_ANDROID_SMOKE_WORLD") == Some("1") {
        args.extend(
            ["--world", "Android Smoke Test", "--seed", "0", "--creative"].map(str::to_owned),
        );
    }
    args
}

// The Android loader looks up this exact symbol. It is called on the native
// activity thread; no raw pointers or JNI ownership cross this boundary.
#[unsafe(no_mangle)]
fn android_main(activity: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("Horizon")
            .with_max_level(log::LevelFilter::Info),
    );
    use winit::platform::android::activity::WindowManagerFlags;
    activity.set_window_flags(
        WindowManagerFlags::FULLSCREEN | WindowManagerFlags::KEEP_SCREEN_ON,
        WindowManagerFlags::FORCE_NOT_FULLSCREEN,
    );
    let Some(mut root) = activity
        .external_data_path()
        .or_else(|| activity.internal_data_path())
    else {
        log::error!("Android did not provide an app data directory");
        return;
    };
    if RESOURCES.set(root.join("resources")).is_err() {
        return;
    }
    // Agent validation uses a fresh app-private folder on every launch.
    // Release/player builds keep their normal app data directory.
    if option_env!("HORIZON_ANDROID_AGENT_SANDBOX") == Some("1") {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        root = root.join("agent-runs").join(nonce.to_string());
        for name in [
            "launcher",
            "xdg-data",
            "xdg-config",
            "trash",
            "clipboard",
            "desktop",
        ] {
            if let Err(error) = std::fs::create_dir_all(root.join(name)) {
                log::error!("Cannot create agent sandbox: {error}");
                return;
            }
        }
    }
    if APP.set(activity).is_err() || ROOT.set(root.clone()).is_err() {
        log::error!("Android activity initialized twice");
        return;
    }
    if let Err(error) = std::fs::create_dir_all(root.join("game")) {
        log::error!("Cannot create game folder: {error}");
        return;
    }
    crate::logging::set_game_home(root.join("game"));
    // First-run mobile defaults only: subsequent player choices persist.
    let settings = root.join("game/options.txt");
    if !settings.exists() {
        let mut mobile = crate::settings::Settings::default();
        crate::graphics::apply_preset(crate::graphics::Preset::Performance, &mut mobile);
        // The desktop Performance preset increases the view radius. Phones
        // keep the smaller world budget while using its cheaper visual options.
        mobile.render_distance = 4;
        // Vanilla's options parser accepts 5..=32; 4 would reload as 12.
        mobile.simulation_distance = 5;
        mobile.max_fps = 60;
        mobile.vsync = true;
        if let Err(error) = mobile.file.set_text("tutorialStep", "none") {
            log::warn!("Cannot disable keyboard tutorial: {error}");
        }
        if let Err(error) = std::fs::write(&settings, mobile.serialize()) {
            log::warn!("Cannot write mobile defaults: {error}");
        }
    }
    let graphics = root.join("game/horizon-graphics.txt");
    if !graphics.exists() {
        let api = option_env!("HORIZON_ANDROID_GRAPHICS_API").unwrap_or("auto");
        // Scale only the world. HUD glyphs, menus and touch hit targets keep
        // the native window resolution. Existing player choices persist.
        let scale = option_env!("HORIZON_ANDROID_RESOLUTION_SCALE")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(50)
            .clamp(50, 100);
        let mobile = crate::graphics::Graphics {
            api: crate::graphics::GraphicsApi::from_name(api).unwrap_or_default(),
            resolution_scale: scale,
            ..Default::default()
        };
        if let Err(error) = std::fs::write(graphics, mobile.serialize()) {
            log::warn!("Cannot write mobile graphics API: {error}");
        }
    }
    let outcome = match crate::game::run() {
        Ok(()) => "Game exited normally\n".to_owned(),
        Err(error) => {
            log::error!("Game stopped: {error}");
            format!("Game stopped: {error}\n")
        }
    };
    // Some OEM builds suppress third-party logcat tags. Keep the startup
    // outcome beside this run's options so a phone-only failure is inspectable.
    if let Err(error) = std::fs::write(root.join("game/startup-result.log"), outcome) {
        log::warn!("Cannot write startup result: {error}");
    }
    // Winit 0.30 permits only one event loop per process. After the game has
    // completed its save/shutdown path, end this NativeActivity-only process
    // so reopening the launcher cannot reuse stale activity/event-loop state.
    std::process::exit(0);
}
