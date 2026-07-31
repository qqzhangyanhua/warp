use pathfinder_geometry::vector::vec2f;
use warp_core::ui::appearance::Appearance;
use warpui::elements::{ConstrainedBox, Expanded, Flex, ParentElement};
use warpui::platform::WindowStyle;
use warpui::{
    App, Element, Entity, Presenter, SingletonEntity, TypedActionView, View, WindowInvalidation,
};

use super::render_file_backed_content;

struct FileBackedRuleRowTestView;

impl Entity for FileBackedRuleRowTestView {
    type Event = ();
}

impl View for FileBackedRuleRowTestView {
    fn ui_name() -> &'static str {
        "FileBackedRuleRowTestView"
    }

    fn render(&self, app: &warpui::AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let content = render_file_backed_content(
            "/a/long/project/path/AGENTS.md".to_string(),
            Some("Use the repository instructions for this project.".to_string()),
            appearance,
        );

        Flex::column()
            .with_child(
                Flex::row()
                    .with_child(Expanded::new(1., content).finish())
                    .with_child(
                        ConstrainedBox::new(Flex::row().finish())
                            .with_width(64.)
                            .finish(),
                    )
                    .finish(),
            )
            .finish()
    }
}

impl TypedActionView for FileBackedRuleRowTestView {
    type Action = ();
}

#[test]
fn file_backed_rule_content_lays_out_with_unbounded_height_without_panicking() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| Appearance::mock());

        let (window_id, _view) =
            app.add_window(WindowStyle::NotStealFocus, |_| FileBackedRuleRowTestView);
        let root_view_id = app
            .root_view_id(window_id)
            .expect("window should have a root view");
        let mut presenter = Presenter::new(window_id);
        let invalidation = WindowInvalidation {
            updated: [root_view_id].into_iter().collect(),
            ..Default::default()
        };

        app.update(move |ctx| {
            presenter.invalidate(invalidation, ctx);
            // A vertical Flex measures non-flexible children with unbounded height,
            // matching the Rules pane's vertical scrollable layout.
            presenter.build_scene(vec2f(400., 400.), 1., None, ctx);
        });
    });
}
