use std::path::PathBuf;

use super::resolve_runtime_program;

#[test]
fn packaged_runtime_uses_the_bundled_bridge() {
    assert_eq!(
        resolve_runtime_program(
            None,
            Some(PathBuf::from("/app/resources")),
            /* release_bundle */ true,
        ),
        Some(
            PathBuf::from("/app/resources")
                .join("bundled")
                .join("agent-runtime")
                .join(if cfg!(windows) {
                    "warp-bridge.exe"
                } else {
                    "warp-bridge"
                })
        )
    );
}

#[test]
fn release_bundle_rejects_a_local_bridge_override() {
    assert_eq!(
        resolve_runtime_program(
            Some(PathBuf::from("/tmp/local-bridge")),
            Some(PathBuf::from("/app/resources")),
            /* release_bundle */ true,
        ),
        None
    );
}

#[test]
fn development_build_accepts_an_explicit_bridge_override() {
    assert_eq!(
        resolve_runtime_program(
            Some(PathBuf::from("/tmp/local-bridge")),
            None,
            /* release_bundle */ false,
        ),
        Some(PathBuf::from("/tmp/local-bridge"))
    );
}
