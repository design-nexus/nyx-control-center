//! The shared Nyx theme with the Control Center's sheet. It never chooses a theme: it
//! follows the one every Nyx app shares and Omarchy's.

use nyx_theme::{Style, ThemeMode, ThemePrefs};

pub fn install() {
    nyx_theme::use_design_2();
    nyx_theme::use_shell_scope();
    nyx_theme::install(Style {
        window_class: "nyx-control-center",
        base_skip: &[],
        css: include_str!("style.css"),
        no_glow_css: "",
        extra_colors: Some(swatch_colors),
        prefs: || {
            let suite = nyx_theme::read_suite();
            ThemePrefs {
                mode: suite.as_ref().map(|s| s.mode.clone()).unwrap_or(ThemeMode::Omarchy),
                theme: suite.map(|s| s.theme).unwrap_or_default(),
                glow: false,
                reduce_motion: false,
            }
        },
        set_theme: None,
    });
}

/// The theme's swatches as `@cc_blue`, `@cc_green`… for the tiles' colours. Every name
/// is defined (the accent stands in for one a theme lacks), so no rule breaks.
fn swatch_colors(p: &nyx_theme::Palette) -> String {
    let swatches = nyx_theme::swatches();
    ["blue", "violet", "pink", "red", "coral", "amber", "green", "teal"]
        .iter()
        .map(|name| {
            let c = swatches.iter().find(|(n, _)| n == name).map(|(_, c)| c.clone()).unwrap_or_else(|| p.accent.clone());
            format!("@define-color cc_{name} {c};\n")
        })
        .collect()
}
