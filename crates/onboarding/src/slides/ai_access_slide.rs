use ui_components::{button, Component as _, Options as _};
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::color::internal_colors;
use warp_core::ui::theme::Fill;
use warpui_core::elements::{
    Border, ClippedScrollStateHandle, Container, CornerRadius, CrossAxisAlignment, Flex,
    FormattedTextElement, Hoverable, MainAxisSize, MouseStateHandle, ParentElement, Radius,
};
use warpui_core::fonts::Weight;
use warpui_core::keymap::Keystroke;
use warpui_core::platform::Cursor;
use warpui_core::prelude::Align;
use warpui_core::text_layout::TextAlignment;
use warpui_core::ui_components::components::{UiComponent as _, UiComponentStyles};
use warpui_core::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity as _, TypedActionView, View,
    ViewContext,
};

use super::OnboardingSlide;
use crate::i18n::{self, Locale, OnboardingMessage};
use crate::model::{AiAccessChoice, OnboardingStateModel};
use crate::slides::{bottom_nav, layout, slide_content};

#[derive(Debug, Clone)]
pub enum AiAccessSlideAction {
    SelectSetUpLater,
    BackClicked,
    NextClicked,
}

/// The "Choose how to access AI" slide (Warp Agent path). Forks between a paid
/// subscription and a "Set up later" option that lets the user explore Warp's
/// built-in AI before committing to a plan.
pub struct AiAccessSlide {
    onboarding_state: ModelHandle<OnboardingStateModel>,
    locale: Locale,
    set_up_later_mouse_state: MouseStateHandle,
    back_button: button::Button,
    next_button: button::Button,
    scroll_state: ClippedScrollStateHandle,
}

impl AiAccessSlide {
    pub(crate) fn new(onboarding_state: ModelHandle<OnboardingStateModel>, locale: Locale) -> Self {
        Self {
            onboarding_state,
            locale,
            set_up_later_mouse_state: MouseStateHandle::default(),
            back_button: button::Button::default(),
            next_button: button::Button::default(),
            scroll_state: ClippedScrollStateHandle::new(),
        }
    }

    // The final DES-816 visual exports have not landed yet, so the right panel
    // reuses the existing bundled agent welcome image.
    pub(crate) const VISUAL_IMAGE_PATHS: &'static [&'static str] =
        &["async/png/onboarding/welcome_agent.png"];

    fn choice(&self, app: &AppContext) -> AiAccessChoice {
        self.onboarding_state.as_ref(app).ai_access_choice()
    }

    fn render_content(
        &self,
        appearance: &Appearance,
        choice: AiAccessChoice,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let bottom_nav = Align::new(self.render_bottom_nav(appearance, app)).finish();

        slide_content::onboarding_slide_content(
            vec![
                Align::new(self.render_header(appearance)).left().finish(),
                Align::new(self.render_options(appearance, choice)).finish(),
            ],
            bottom_nav,
            self.scroll_state.clone(),
            appearance,
        )
    }

    fn render_header(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();

        let title = appearance
            .ui_builder()
            .paragraph(i18n::tr(OnboardingMessage::ConfigureAi, self.locale))
            .with_style(UiComponentStyles {
                font_size: Some(36.),
                font_weight: Some(Weight::Medium),
                ..Default::default()
            })
            .build()
            .finish();

        let subtitle = FormattedTextElement::from_str(
            i18n::tr(OnboardingMessage::AiAccessSubtitleAnonymous, self.locale),
            appearance.ui_font_family(),
            16.,
        )
        .with_color(internal_colors::text_sub(
            theme,
            theme.background().into_solid(),
        ))
        .with_weight(Weight::Normal)
        .with_alignment(TextAlignment::Left)
        .with_line_height_ratio(1.0)
        .finish();

        Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(title)
            .with_child(Container::new(subtitle).with_margin_top(16.).finish())
            .finish()
    }

    fn render_options(&self, appearance: &Appearance, choice: AiAccessChoice) -> Box<dyn Element> {
        let _ = choice;
        Container::new(self.render_set_up_later_card(appearance, true))
            .with_margin_top(38.)
            .finish()
    }

    /// Shared chrome for an option card: selected/unselected background + border,
    /// hover/click to select.
    fn render_card_chrome(
        appearance: &Appearance,
        is_selected: bool,
        mouse_state: MouseStateHandle,
        select_action: AiAccessSlideAction,
        content: Box<dyn Element>,
    ) -> Box<dyn Element> {
        const RADIUS: f32 = 8.;

        let theme = appearance.theme();
        let background = if is_selected {
            Some(internal_colors::accent_overlay_1(theme))
        } else {
            None
        };
        let border_color = if is_selected {
            theme.accent()
        } else {
            Fill::Solid(internal_colors::neutral_4(theme))
        };

        Hoverable::new(mouse_state, move |_| {
            let mut container = Container::new(content)
                .with_uniform_padding(24.)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(RADIUS)))
                .with_border(Border::all(1.).with_border_fill(border_color));
            if let Some(bg) = background {
                container = container.with_background(bg);
            }
            container.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(select_action.clone());
        })
        .finish()
    }

    fn render_set_up_later_card(
        &self,
        appearance: &Appearance,
        is_selected: bool,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let bg_solid = theme.background().into_solid();
        let label_color = if is_selected {
            internal_colors::text_main(theme, bg_solid)
        } else {
            internal_colors::text_sub(theme, bg_solid)
        };
        let description_color = internal_colors::text_sub(theme, bg_solid);

        let label = appearance
            .ui_builder()
            .paragraph(i18n::tr(OnboardingMessage::SetUpLater, self.locale))
            .with_style(UiComponentStyles {
                font_size: Some(16.),
                font_weight: Some(Weight::Semibold),
                font_color: Some(label_color),
                ..Default::default()
            })
            .build()
            .finish();

        let description = FormattedTextElement::from_str(
            "Explore Warp's built-in AI features before committing to a plan, or bring your own \
             inference.",
            appearance.ui_font_family(),
            14.,
        )
        .with_color(description_color)
        .with_weight(Weight::Normal)
        .with_alignment(TextAlignment::Left)
        .with_line_height_ratio(1.2)
        .finish();

        let content = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(label)
            .with_child(Container::new(description).with_margin_top(12.).finish())
            .finish();

        Self::render_card_chrome(
            appearance,
            is_selected,
            self.set_up_later_mouse_state.clone(),
            AiAccessSlideAction::SelectSetUpLater,
            content,
        )
    }

    fn render_bottom_nav(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let back_button = self.back_button.render(
            appearance,
            button::Params {
                content: button::Content::Label(
                    i18n::tr(OnboardingMessage::Back, self.locale).into(),
                ),
                theme: &button::themes::Naked,
                options: button::Options {
                    on_click: Some(Box::new(|ctx, _app, _pos| {
                        ctx.dispatch_typed_action(AiAccessSlideAction::BackClicked);
                    })),
                    ..button::Options::default(appearance)
                },
            },
        );

        let enter = Keystroke::parse("enter").unwrap_or_default();
        let next_button = self.next_button.render(
            appearance,
            button::Params {
                content: button::Content::Label(
                    i18n::tr(OnboardingMessage::Next, self.locale).into(),
                ),
                theme: &button::themes::Primary,
                options: button::Options {
                    keystroke: Some(enter),
                    on_click: Some(Box::new(|ctx, _app, _pos| {
                        ctx.dispatch_typed_action(AiAccessSlideAction::NextClicked);
                    })),
                    ..button::Options::default(appearance)
                },
            },
        );

        let (step_index, step_count) = self.onboarding_state.as_ref(app).progress();
        bottom_nav::onboarding_bottom_nav(
            appearance,
            step_index,
            step_count,
            Some(back_button),
            Some(next_button),
        )
    }

    fn render_visual(&self) -> Box<dyn Element> {
        layout::onboarding_right_panel_with_bg(
            Self::VISUAL_IMAGE_PATHS[0],
            layout::FOREGROUND_LAYOUT_DEFAULT,
        )
    }
}

impl Entity for AiAccessSlide {
    type Event = ();
}

impl View for AiAccessSlide {
    fn ui_name() -> &'static str {
        "AiAccessSlide"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let choice = self.choice(app);

        layout::static_left(
            || self.render_content(appearance, choice, app),
            || self.render_visual(),
        )
    }
}

impl AiAccessSlide {
    fn select_choice(&mut self, choice: AiAccessChoice, ctx: &mut ViewContext<Self>) {
        self.onboarding_state.update(ctx, |model, ctx| {
            model.set_ai_access_choice(choice, ctx);
        });
        ctx.notify();
    }

    fn next(&mut self, ctx: &mut ViewContext<Self>) {
        self.onboarding_state.update(ctx, |model, ctx| {
            model.next(ctx);
        });
    }

    fn advance_or_upgrade(&mut self, ctx: &mut ViewContext<Self>) {
        self.select_choice(AiAccessChoice::SetUpLater, ctx);
        self.next(ctx);
    }
}

impl OnboardingSlide for AiAccessSlide {
    fn on_up(&mut self, ctx: &mut ViewContext<Self>) {
        self.select_choice(AiAccessChoice::SetUpLater, ctx);
    }

    fn on_down(&mut self, ctx: &mut ViewContext<Self>) {
        self.select_choice(AiAccessChoice::SetUpLater, ctx);
    }

    fn on_enter(&mut self, ctx: &mut ViewContext<Self>) {
        self.advance_or_upgrade(ctx);
    }
}

impl TypedActionView for AiAccessSlide {
    type Action = AiAccessSlideAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            AiAccessSlideAction::SelectSetUpLater => {
                self.select_choice(AiAccessChoice::SetUpLater, ctx);
            }
            AiAccessSlideAction::BackClicked => {
                self.onboarding_state.update(ctx, |model, ctx| {
                    model.back(ctx);
                });
            }
            AiAccessSlideAction::NextClicked => {
                self.advance_or_upgrade(ctx);
            }
        }
    }
}
