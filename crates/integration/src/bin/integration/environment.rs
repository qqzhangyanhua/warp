use std::env;
use std::path::Path;

pub(super) fn validate_isolation() {
    let zyh_home = env::var("ZYH_HOME")
        .expect("Integration test binary should have set a ZYH_HOME environment variable");
    assert!(
        Path::new(&zyh_home).ends_with("zyh-home"),
        "ZYH_HOME should point to the isolated integration root"
    );

    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            let home = env::var("HOME")
                .expect("Should have a value for the HOME environment variable");
            let original_home = env::var("ORIGINAL_HOME").expect(
                "Integration test binary should have set an ORIGINAL_HOME environment variable",
            );
            assert_ne!(home, original_home, "HOME should not be the same as ORIGINAL_HOME!");
        } else {
            unimplemented!("Need to add support for hermetic integration tests for the current platform!");
        }
    }
}
