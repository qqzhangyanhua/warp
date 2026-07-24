//! The headless `warp-tui` front-end's app-side entry point.

use warpui::AppContext;

use crate::TuiMountFn;

/// Entry point invoked from `run_internal` once the headless app is initialized.
///
/// Mounts the permanent local-product TUI after shared initialization.
pub(crate) fn init(mount: TuiMountFn, ctx: &mut AppContext) {
    mount(ctx);
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
