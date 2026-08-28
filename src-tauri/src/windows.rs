use tauri::{
    AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    menu::{Menu, MenuBuilder, SubmenuBuilder},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowSurface {
    Library,
    Artwork,
    Queue,
    Mini,
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
    },
    WindowSpec {
        kind: WindowSurface::Artwork,
        label: "artwork",
        title: "Artwork",
        route: "?view=artwork",
        width: 480.0,
        height: 650.0,
        decorations: true,
        transparent: true,
        resizable: true,
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
    },
    WindowSpec {
        kind: WindowSurface::Mini,
        label: "mini-player",
        title: "G Music Mini Player",
        route: "?view=mini",
        width: 520.0,
        height: 174.0,
        decorations: false,
        transparent: true,
        resizable: false,
    },
];

const MENU_TITLES: [&str; 6] = ["G Music", "File", "Edit", "View", "Window", "Help"];

#[cfg(test)]
fn all_surfaces() -> &'static [WindowSpec] {
    SURFACES
}

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let application = SubmenuBuilder::new(app, MENU_TITLES[0])
        .about(None)
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
    let view = SubmenuBuilder::new(app, MENU_TITLES[3])
        .fullscreen()
        .build()?;
    let window = SubmenuBuilder::new(app, MENU_TITLES[4])
        .minimize()
        .maximize()
        .separator()
        .text("window.library", "Library")
        .text("window.artwork", "Artwork")
        .text("window.queue", "Play Queue")
        .separator()
        .text("window.mini", "Mini Player")
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = SubmenuBuilder::new(app, MENU_TITLES[5]).build()?;

    MenuBuilder::new(app)
        .item(&application)
        .item(&file)
        .item(&edit)
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
        _ => return,
    };

    if let Err(error) = result {
        eprintln!("failed to handle native menu item {item_id}: {error}");
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
            eprintln!(
                "failed to apply native vibrancy to window {}: {error}",
                window.label()
            );
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

    let window = WebviewWindowBuilder::new(
        app,
        spec.label,
        WebviewUrl::App(format!("index.html{}", spec.route).into()),
    )
    .title(spec.title)
    .inner_size(spec.width, spec.height)
    .min_inner_size(spec.width.min(520.0), spec.height.min(360.0))
    .decorations(spec.decorations)
    .transparent(spec.transparent)
    .resizable(spec.resizable)
    .build()?;

    apply_native_glass(&window);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MENU_TITLES, WindowSurface, all_surfaces};

    #[test]
    fn native_menu_uses_standard_macos_sections() {
        assert_eq!(
            MENU_TITLES,
            ["G Music", "File", "Edit", "View", "Window", "Help"]
        );
    }

    #[test]
    fn desktop_surfaces_use_distinct_window_labels_and_routes() {
        let surfaces = all_surfaces();

        assert_eq!(surfaces.len(), 4);
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
        assert!(surfaces.iter().all(|surface| surface.label != "albums"));
        assert!(surfaces.iter().all(|surface| surface.label != "artists"));
        assert!(surfaces.iter().all(|surface| surface.transparent));
    }
}
