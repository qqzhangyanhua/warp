use warp_core::features::FeatureFlag;
use warpui::App;

use super::ImprovedPaletteSearch;

#[test]
#[serial_test::serial]
fn local_only_disables_search_experiment_without_initializing_experiments() {
    let _flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        assert!(!app.update(ImprovedPaletteSearch::improved_search_enabled));
    });
}
