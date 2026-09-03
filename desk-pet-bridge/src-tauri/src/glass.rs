use tauri::WebviewWindow;

const CORNER_RADIUS: f64 = 16.0;

pub fn apply_menu_glass(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    apply_macos_menu_glass(window);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_menu_glass(window: &WebviewWindow) {
    use objc2_app_kit::NSView;
    use window_vibrancy::{
        apply_liquid_glass, apply_vibrancy, LiquidGlassOptions, NSGlassEffectViewStyle,
        NSVisualEffectMaterial, NSVisualEffectState,
    };

    let window_for_glass = window.clone();
    let applied = window.with_webview(move |webview| {
        let ns_view: &NSView = unsafe { &*webview.inner().cast() };
        let options = LiquidGlassOptions::new(NSGlassEffectViewStyle::Regular)
            .radius(CORNER_RADIUS)
            .content_view(ns_view);

        if let Err(err) = apply_liquid_glass(&window_for_glass, options) {
            tracing::info!(
                error = %err,
                "liquid glass unavailable; using menu-bar popover vibrancy"
            );
            if let Err(err) = apply_vibrancy(
                &window_for_glass,
                NSVisualEffectMaterial::Popover,
                Some(NSVisualEffectState::Active),
                Some(CORNER_RADIUS),
            ) {
                tracing::warn!(error = %err, "failed to apply popover vibrancy");
            }
        }
    });

    if applied.is_err() {
        fallback_vibrancy(window);
    }
}

#[cfg(target_os = "macos")]
fn fallback_vibrancy(window: &WebviewWindow) {
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

    if let Err(err) = apply_vibrancy(
        window,
        NSVisualEffectMaterial::Popover,
        Some(NSVisualEffectState::Active),
        Some(CORNER_RADIUS),
    ) {
        tracing::warn!(error = %err, "failed to apply popover vibrancy");
    }
}
