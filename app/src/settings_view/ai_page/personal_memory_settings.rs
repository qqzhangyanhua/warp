use warpui::elements::{Border, Container, CrossAxisAlignment, Flex, ParentElement, Text};
use warpui::fonts::{Properties, Weight};
use warpui::{AppContext, Element, SingletonEntity, ViewContext};

use super::personal_memory_embedding_settings::render_personal_memory_embedding_controls;
use super::personal_memory_toggle::PersonalMemoryToggle;
use super::{AISettingsPageView, CONTENT_FONT_SIZE};
use crate::ai::personal_memory::{
    PersonalMemoryRecord, PersonalMemoryService, PERSONAL_MEMORY_RECORD_LIMIT,
};
use crate::appearance::Appearance;
use crate::i18n::{tr, tr_cached, Message};
use crate::settings_view::settings_page::{build_sub_header, SettingsWidget, HEADER_PADDING};
use crate::GlobalResourceHandlesProvider;

#[derive(Default)]
pub(super) enum PersonalMemorySettingsState {
    #[default]
    NotLoaded,
    Loading,
    Loaded(Vec<PersonalMemoryRecord>),
    Error,
}

#[derive(Default)]
pub(super) struct PersonalMemoryWidget {
    enabled_toggle: PersonalMemoryToggle,
}
impl AISettingsPageView {
    pub(crate) fn focus_personal_memory_record(
        &mut self,
        record_id: String,
        ctx: &mut ViewContext<Self>,
    ) {
        self.personal_memory_focus_record_id = Some(record_id);
        if self.active_subpage != Some(super::AISubpage::PersonalMemory) {
            self.set_active_subpage(Some(super::AISubpage::PersonalMemory), ctx);
        } else {
            self.refresh_personal_memory(ctx);
            ctx.notify();
        }
    }

    pub(super) fn refresh_personal_memory(&mut self, ctx: &mut ViewContext<Self>) {
        if matches!(
            self.personal_memory_state,
            PersonalMemorySettingsState::Loading
        ) {
            return;
        }
        let Some(sender) = GlobalResourceHandlesProvider::as_ref(ctx)
            .get()
            .model_event_sender
            .clone()
        else {
            self.personal_memory_state = PersonalMemorySettingsState::Error;
            ctx.notify();
            return;
        };

        self.personal_memory_state = PersonalMemorySettingsState::Loading;
        ctx.notify();
        ctx.spawn(
            async move { PersonalMemoryService::new(sender).list().await },
            |view, result, ctx| {
                view.personal_memory_state = match result {
                    Ok(records) => PersonalMemorySettingsState::Loaded(records),
                    Err(_) => PersonalMemorySettingsState::Error,
                };
                ctx.notify();
            },
        );
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn focused_personal_memory_record_has_value_for_test(
        &self,
        expected_value: &str,
    ) -> bool {
        let Some(focused_record_id) = self.personal_memory_focus_record_id.as_deref() else {
            return false;
        };
        let PersonalMemorySettingsState::Loaded(records) = &self.personal_memory_state else {
            return false;
        };
        records.iter().any(|record| {
            record.record_id == focused_record_id && record.value_text == expected_value
        })
    }
}

impl SettingsWidget for PersonalMemoryWidget {
    type View = AISettingsPageView;

    fn search_terms(&self) -> &str {
        "personal memory memories remembered facts local records"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let header = build_sub_header(
            appearance,
            tr(app, Message::SettingsSectionPersonalMemory),
            None,
        )
        .with_margin_bottom(HEADER_PADDING)
        .finish();
        Flex::column()
            .with_child(header)
            .with_child(self.enabled_toggle.render(view, app))
            .with_child(render_personal_memory_embedding_controls(
                &view.personal_memory_embedding_controls,
                appearance,
                app,
            ))
            .with_child(render_state(
                &view.personal_memory_state,
                view.personal_memory_focus_record_id.as_deref(),
                appearance,
            ))
            .finish()
    }
}

fn render_state(
    state: &PersonalMemorySettingsState,
    focused_record_id: Option<&str>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    match state {
        PersonalMemorySettingsState::NotLoaded | PersonalMemorySettingsState::Loading => {
            status_text(tr_cached(Message::PersonalMemoryLoading), appearance)
        }
        PersonalMemorySettingsState::Error => {
            status_text(tr_cached(Message::PersonalMemoryLoadFailed), appearance)
        }
        PersonalMemorySettingsState::Loaded(records) if records.is_empty() => {
            status_text(tr_cached(Message::PersonalMemoryEmpty), appearance)
        }
        PersonalMemorySettingsState::Loaded(records) => {
            render_records(records, focused_record_id, appearance)
        }
    }
}

fn status_text(text: impl Into<String>, appearance: &Appearance) -> Box<dyn Element> {
    Text::new(text.into(), appearance.ui_font_family(), CONTENT_FONT_SIZE)
        .with_color(appearance.theme().nonactive_ui_text_color().into())
        .finish()
}

fn render_records(
    records: &[PersonalMemoryRecord],
    focused_record_id: Option<&str>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let count = tr_cached(Message::PersonalMemoryRecordCount)
        .replace("{count}", &records.len().to_string())
        .replace("{limit}", &PERSONAL_MEMORY_RECORD_LIMIT.to_string());
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(
            Container::new(
                Text::new(count, appearance.ui_font_family(), CONTENT_FONT_SIZE)
                    .with_color(theme.nonactive_ui_text_color().into())
                    .finish(),
            )
            .with_padding_bottom(8.)
            .finish(),
        );

    for record in records_for_render(records, focused_record_id) {
        let is_focused = focused_record_id == Some(record.record_id.as_str());
        let border_fill = if is_focused {
            theme.accent()
        } else {
            theme.outline()
        };
        column.add_child(
            Container::new(render_record(record, appearance))
                .with_vertical_padding(12.)
                .with_horizontal_padding(if is_focused { 8. } else { 0. })
                .with_border(Border::bottom(1.).with_border_fill(border_fill))
                .finish(),
        );
    }
    column.finish()
}

fn records_for_render<'a>(
    records: &'a [PersonalMemoryRecord],
    focused_record_id: Option<&str>,
) -> Vec<&'a PersonalMemoryRecord> {
    let focused = focused_record_id
        .and_then(|record_id| records.iter().find(|record| record.record_id == record_id));
    focused
        .into_iter()
        .chain(
            records
                .iter()
                .filter(|record| Some(record.record_id.as_str()) != focused_record_id),
        )
        .collect()
}

fn render_record(record: &PersonalMemoryRecord, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let topic = Text::new(
        record.topic.clone(),
        appearance.ui_font_family(),
        CONTENT_FONT_SIZE,
    )
    .with_style(Properties::default().weight(Weight::Bold))
    .with_color(theme.active_ui_text_color().into())
    .finish();
    let value = Text::new(
        record.value_text.clone(),
        appearance.monospace_font_family(),
        CONTENT_FONT_SIZE,
    )
    .with_color(theme.active_ui_text_color().into())
    .finish();
    let fact = Text::new(
        record.fact_text.clone(),
        appearance.ui_font_family(),
        CONTENT_FONT_SIZE,
    )
    .with_color(theme.nonactive_ui_text_color().into())
    .finish();
    let updated_at = Text::new(
        tr_cached(Message::PersonalMemoryUpdatedAt).replace(
            "{time}",
            &record.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        ),
        appearance.ui_font_family(),
        CONTENT_FONT_SIZE - 1.,
    )
    .with_color(theme.disabled_ui_text_color().into())
    .finish();

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(topic)
        .with_child(Container::new(value).with_padding_top(4.).finish())
        .with_child(Container::new(fact).with_padding_top(4.).finish())
        .with_child(Container::new(updated_at).with_padding_top(6.).finish())
        .finish()
}
