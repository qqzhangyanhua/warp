use warpui::elements::{Container, CornerRadius, Element, Empty, Radius, Text};

use crate::appearance::Appearance;
use crate::i18n::{tr_cached, Message};
use crate::themes::theme::Fill;

pub fn create_discount_badge(discount: u32, appearance: &Appearance) -> Box<dyn Element> {
    if discount == 0 {
        return Empty::new().finish();
    }

    let theme = appearance.theme();
    let background: Fill = theme.terminal_colors().normal.green.into();

    Container::new(
        Text::new_inline(
            tr_cached(Message::BillingPercentOff).replace("{discount}", &discount.to_string()),
            appearance.ui_font_family(),
            10.,
        )
        .with_color(theme.main_text_color(background).into())
        .finish(),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
    .with_background(background)
    .with_uniform_padding(4.)
    .finish()
}
