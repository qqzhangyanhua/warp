use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use warpui::App;

use super::*;

#[test]
fn zyh_tui_mounts_without_identity_state() {
    let mounted = Arc::new(AtomicBool::new(false));
    let mounted_from_callback = mounted.clone();

    App::test((), |mut app| async move {
        app.update(|ctx| {
            init(
                Box::new(move |_| mounted_from_callback.store(true, Ordering::Release)),
                ctx,
            );
        });

        assert!(mounted.load(Ordering::Acquire));
    });
}
