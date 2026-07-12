use warpui::keymap::{BindingDescription, PerPlatformKeystroke};

use super::StaticCommand;
use crate::search::slash_command_menu::static_commands::localized_command_description;

pub enum DefaultSlashCommandBinding {
    None,
    Single(&'static str),
    PerPlatform(PerPlatformKeystroke),
}

pub fn default_binding_for_command(name: &'static str) -> DefaultSlashCommandBinding {
    match name {
        "/agent" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "cmd-enter",
            linux_and_windows: "ctrl-shift-enter",
        }),
        "/cloud-agent" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "cmd-alt-enter",
            linux_and_windows: "ctrl-alt-enter",
        }),
        "/conversations" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "cmd-y",
            linux_and_windows: "ctrl-shift-Y",
        }),
        "/open-repo" => DefaultSlashCommandBinding::PerPlatform(PerPlatformKeystroke {
            mac: "alt-cmd-o",
            linux_and_windows: "ctrl-alt-o",
        }),
        _ => DefaultSlashCommandBinding::None,
    }
}

pub fn binding_description(command: &StaticCommand) -> BindingDescription {
    let name = command.name;
    let description = command.description;
    BindingDescription::new_preserve_case(format!("Slash command: {name}")).with_dynamic_override(
        move |ctx| {
            if crate::i18n::active_locale(ctx) != crate::i18n::Locale::ZhCn {
                return None;
            }

            Some(format!(
                "斜杠命令：{name} — {}",
                localized_command_description(ctx, description)
            ))
        },
    )
}
