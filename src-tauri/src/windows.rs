use tauri::{
    AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowSurface {
    Library,
    Artwork,
    Queue,
    Mini,
    Settings,
    Import,
}

#[derive(Clone, Copy, Debug)]
struct WindowSpec {
    kind: WindowSurface,
    label: &'static str,
    title: &'static str,
    route: &'static str,
    width: f64,
    height: f64,
    decorations: bool,
    transparent: bool,
    resizable: bool,
    overlay_titlebar: bool,
    hidden_title: bool,
}

const SURFACES: &[WindowSpec] = &[
    WindowSpec {
        kind: WindowSurface::Library,
        label: "main",
        title: "G Music",
        route: "",
        width: 1040.0,
        height: 660.0,
        decorations: true,
        transparent: true,
        resizable: true,
        overlay_titlebar: true,
        hidden_title: true,
    },
    WindowSpec {
        kind: WindowSurface::Artwork,
        label: "artwork",
        title: "Artwork",
        route: "?view=artwork",
        width: 320.0,
        height: 320.0,
        decorations: true,
        transparent: true,
        resizable: true,
        overlay_titlebar: true,
        hidden_title: true,
    },
    WindowSpec {
        kind: WindowSurface::Queue,
        label: "queue",
        title: "Play Queue",
        route: "?view=queue",
        width: 400.0,
        height: 520.0,
        decorations: true,
        transparent: true,
        resizable: true,
        overlay_titlebar: true,
        hidden_title: true,
    },
    WindowSpec {
        kind: WindowSurface::Mini,
        label: "mini-player",
        title: "G Music Mini Player",
        route: "?view=mini",
        width: 480.0,
        height: 144.0,
        decorations: false,
        transparent: true,
        resizable: false,
        overlay_titlebar: false,
        hidden_title: false,
    },
    WindowSpec {
        kind: WindowSurface::Settings,
        label: "settings",
        title: "Settings",
        route: "?view=settings",
        width: 720.0,
        height: 480.0,
        decorations: true,
        transparent: true,
        resizable: true,
        overlay_titlebar: false,
        hidden_title: false,
    },
    WindowSpec {
        kind: WindowSurface::Import,
        label: "import",
        title: "Import Music",
        route: "?view=import",
        width: 680.0,
        height: 620.0,
        decorations: true,
        transparent: true,
        resizable: true,
        overlay_titlebar: true,
        hidden_title: true,
    },
];

const MENU_TITLES: [&str; 7] = [
    "G Music", "File", "Edit", "Playback", "View", "Window", "Help",
];

#[cfg(test)]
fn all_surfaces() -> &'static [WindowSpec] {
    SURFACES
}

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let settings = MenuItemBuilder::with_id("window.settings", "Settings…")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;
    let import = MenuItemBuilder::with_id("window.import", "Import Music…")
        .accelerator("CmdOrCtrl+I")
        .build(app)?;
    let toggle_playback = MenuItemBuilder::with_id("playback.toggle", "Play / Pause")
        .accelerator("Space")
        .build(app)?;
    let previous_track = MenuItemBuilder::with_id("playback.previous", "Previous Track")
        .accelerator("CmdOrCtrl+Left")
        .build(app)?;
    let next_track = MenuItemBuilder::with_id("playback.next", "Next Track")
        .accelerator("CmdOrCtrl+Right")
        .build(app)?;
    let library = MenuItemBuilder::with_id("window.library", "Library")
        .accelerator("CmdOrCtrl+1")
        .build(app)?;
    let artwork = MenuItemBuilder::with_id("window.artwork", "Artwork")
        .accelerator("CmdOrCtrl+2")
        .build(app)?;
    let queue = MenuItemBuilder::with_id("window.queue", "Play Queue")
        .accelerator("CmdOrCtrl+3")
        .build(app)?;
    let mini_player = MenuItemBuilder::with_id("window.mini", "Mini Player")
        .accelerator("CmdOrCtrl+4")
        .build(app)?;
    let keyboard_shortcuts =
        MenuItemBuilder::with_id("help.keyboard-shortcuts", "Keyboard Shortcuts…")
            .accelerator("CmdOrCtrl+/")
            .build(app)?;
    let application = SubmenuBuilder::new(app, MENU_TITLES[0])
        .about(None)
        .separator()
        .item(&settings)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(app, MENU_TITLES[1])
        .item(&import)
        .separator()
        .close_window()
        .build()?;
    let edit = SubmenuBuilder::new(app, MENU_TITLES[2])
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let playback = SubmenuBuilder::new(app, MENU_TITLES[3])
        .item(&toggle_playback)
        .separator()
        .item(&previous_track)
        .item(&next_track)
        .build()?;
    let view = SubmenuBuilder::new(app, MENU_TITLES[4])
        .fullscreen()
        .build()?;
    let window = SubmenuBuilder::new(app, MENU_TITLES[5])
        .minimize()
        .maximize()
        .separator()
        .item(&library)
        .item(&artwork)
        .item(&queue)
        .separator()
        .item(&mini_player)
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = SubmenuBuilder::new(app, MENU_TITLES[6])
        .item(&keyboard_shortcuts)
        .build()?;

    MenuBuilder::new(app)
        .item(&application)
        .item(&file)
        .item(&edit)
        .item(&playback)
        .item(&view)
        .item(&window)
        .item(&help)
        .build()
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, item_id: &str) {
    let result = match item_id {
        "window.library" => show_surface(app, WindowSurface::Library),
        "window.artwork" => show_surface(app, WindowSurface::Artwork),
        "window.queue" => show_surface(app, WindowSurface::Queue),
        "window.mini" => show_surface(app, WindowSurface::Mini),
        "window.settings" => show_surface(app, WindowSurface::Settings),
        "window.import" => show_surface(app, WindowSurface::Import),
        _ => {
            crate::commands::handle_menu_event(app, item_id);
            return;
        }
    };

    if let Err(error) = result {
        tracing::error!(item_id, %error, "failed to handle native menu item");
    }
}

pub fn apply_native_glass<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState, apply_vibrancy};

        let radius = (window.label() == "mini-player").then_some(22.0);
        if let Err(error) = apply_vibrancy(
            window,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::FollowsWindowActiveState),
            radius,
        ) {
            tracing::error!(window = window.label(), %error, "failed to apply native vibrancy");
        }
    }
}

fn show_surface<R: Runtime>(app: &AppHandle<R>, surface: WindowSurface) -> tauri::Result<()> {
    let Some(spec) = SURFACES.iter().find(|spec| spec.kind == surface) else {
        return Ok(());
    };

    if let Some(window) = app.get_webview_window(spec.label) {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        app,
        spec.label,
        WebviewUrl::App(format!("index.html{}", spec.route).into()),
    )
    .title(spec.title)
    .inner_size(spec.width, spec.height)
    .min_inner_size(spec.width.min(520.0), spec.height.min(360.0))
    .decorations(spec.decorations)
    .transparent(spec.transparent)
    .resizable(spec.resizable);

    // Library tracks use DOM drag/drop, not Tauri's native file-drop handler.
    let builder = if surface == WindowSurface::Library {
        builder.disable_drag_drop_handler()
    } else {
        builder
    };

    #[cfg(target_os = "macos")]
    let builder = if spec.overlay_titlebar {
        builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(spec.hidden_title)
    } else {
        builder
    };

    let window = builder.build()?;

    apply_native_glass(&window);

    Ok(())
}

pub fn show_import<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    show_surface(app, WindowSurface::Import)
}

pub fn show_youtube_login<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("youtube-auth") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let login_url = url::Url::parse(
        "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2F",
    )
    .map_err(tauri::Error::InvalidUrl)?;
    WebviewWindowBuilder::new(app, "youtube-auth", WebviewUrl::External(login_url))
        .title("Sign in to YouTube")
        .inner_size(960.0, 720.0)
        .min_inner_size(640.0, 520.0)
        .resizable(true)
        .incognito(true)
        .on_navigation(is_youtube_auth_navigation)
        .build()?;

    Ok(())
}

fn is_youtube_auth_navigation(url: &url::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }

    url.host_str().is_some_and(|host| {
        ["google.com", "youtube.com"]
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    })
}

#[cfg(test)]
mod tests {
    use super::{MENU_TITLES, WindowSurface, all_surfaces, is_youtube_auth_navigation};
    use url::Url;

    #[test]
    fn native_menu_uses_standard_macos_sections() {
        assert_eq!(
            MENU_TITLES,
            [
                "G Music", "File", "Edit", "Playback", "View", "Window", "Help"
            ]
        );
    }

    #[test]
    fn desktop_surfaces_use_distinct_window_labels_and_routes() {
        let surfaces = all_surfaces();

        assert_eq!(surfaces.len(), 6);
        assert!(
            surfaces
                .iter()
                .any(|surface| surface.kind == WindowSurface::Library)
        );
        assert!(
            surfaces
                .iter()
                .any(|surface| surface.route == "?view=artwork")
        );
        assert!(
            surfaces
                .iter()
                .any(|surface| surface.label == "mini-player")
        );
        assert!(
            surfaces
                .iter()
                .any(|surface| surface.route == "?view=settings")
        );
        assert!(surfaces.iter().any(|surface| {
            surface.kind == WindowSurface::Import
                && surface.label == "import"
                && surface.route == "?view=import"
        }));
        assert!(surfaces.iter().all(|surface| surface.label != "albums"));
        assert!(surfaces.iter().all(|surface| surface.label != "artists"));
        assert!(surfaces.iter().all(|surface| surface.transparent));
    }

    #[test]
    fn artwork_uses_a_hidden_overlay_titlebar() {
        let artwork = all_surfaces()
            .iter()
            .find(|surface| surface.kind == WindowSurface::Artwork)
            .expect("artwork surface");

        assert!(artwork.overlay_titlebar);
        assert!(artwork.hidden_title);
        assert!(artwork.decorations);
    }

    #[test]
    fn queue_uses_a_hidden_overlay_titlebar() {
        let queue = all_surfaces()
            .iter()
            .find(|surface| surface.kind == WindowSurface::Queue)
            .expect("queue surface");

        assert!(queue.overlay_titlebar);
        assert!(queue.hidden_title);
        assert!(queue.decorations);
    }

    #[test]
    fn import_uses_the_library_window_chrome() {
        let import = all_surfaces()
            .iter()
            .find(|surface| surface.kind == WindowSurface::Import)
            .expect("import surface");

        assert!(import.overlay_titlebar);
        assert!(import.hidden_title);
        assert!(import.decorations);
        assert!(import.transparent);
    }

    #[test]
    fn youtube_login_navigation_stays_on_google_and_youtube() {
        for url in [
            "https://accounts.google.com/ServiceLogin",
            "https://myaccount.google.com/",
            "https://www.youtube.com/",
            "https://music.youtube.com/",
        ] {
            assert!(is_youtube_auth_navigation(&Url::parse(url).unwrap()));
        }
        assert!(!is_youtube_auth_navigation(
            &Url::parse("https://example.com/").unwrap()
        ));
        assert!(!is_youtube_auth_navigation(
            &Url::parse("http://accounts.google.com/").unwrap()
        ));
    }
}
