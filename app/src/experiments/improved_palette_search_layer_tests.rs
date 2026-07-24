use warpui::App;

use super::ImprovedPaletteSearch;

#[test]
#[serial_test::serial]
fn local_only_disables_search_experiment_without_initializing_experiments() {
    App::test((), |mut app| async move {
        assert!(!app.update(ImprovedPaletteSearch::improved_search_enabled));
    });
}
