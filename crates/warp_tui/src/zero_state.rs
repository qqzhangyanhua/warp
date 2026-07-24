//! The pre-first-interaction "zero state" filling the transcript area: the
//! ZYH Agent title and version, and the session's project context (rules and
//! skills discovered).
//!
//! The session view owns visibility: the zero state fills the transcript
//! slot while the transcript has no visible content, so it dismisses once
//! the first accepted submission produces a block and returns whenever the
//! transcript empties out again.

use std::path::PathBuf;

use ai::project_context::model::ProjectContextModel;
use warp::tui_export::SkillManager;
use warp_core::channel::ChannelState;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warpui::SingletonEntity;
use warpui_core::elements::tui::{Modifier, TuiConstrainedBox, TuiElement, TuiFlex, TuiText};
use warpui_core::AppContext;

use crate::tui_builder::TuiUiBuilder;
use crate::ui::abbreviate_home_prefix;

/// Width cap on the text column so bullets wrap like the mock.
const LEFT_COLUMN_MAX_COLS: u16 = 48;

/// Renders the zero state for the transcript area. `cwd` is the session's
/// working directory for the project section.
pub(crate) fn render_zero_state(cwd: Option<&str>, app: &AppContext) -> Box<dyn TuiElement> {
    let builder = TuiUiBuilder::from_app(app);
    TuiConstrainedBox::new(render_left_column(cwd, &builder, app).finish())
        .with_max_cols(LEFT_COLUMN_MAX_COLS)
        .finish()
}

/// The left text column: title, version, "What's new", and project context.
fn render_left_column(cwd: Option<&str>, builder: &TuiUiBuilder, app: &AppContext) -> TuiFlex {
    let title_style = builder.accent_text_style().add_modifier(Modifier::BOLD);
    let mut column = TuiFlex::column()
        .child(
            TuiText::new("ZYH Agent")
                .with_style(title_style)
                .truncate()
                .finish(),
        )
        .child(render_version_line(builder));

    if let Some(cwd) = cwd {
        column = render_project_section(cwd, column, builder, app);
    }
    column
}

/// The local build version, or "dev build" when no release version is set.
fn render_version_line(builder: &TuiUiBuilder) -> Box<dyn TuiElement> {
    let muted = builder.muted_text_style();
    let Some(version) = ChannelState::app_version() else {
        return TuiText::new("dev build")
            .with_style(muted)
            .truncate()
            .finish();
    };
    TuiText::new(version).with_style(muted).truncate().finish()
}

/// Appends the project section: the project root (or cwd) as a header, then
/// one line per discovered rule file and a discovered-skill count. Discovery
/// is asynchronous, so a placeholder shows until results land.
fn render_project_section(
    cwd: &str,
    mut column: TuiFlex,
    builder: &TuiUiBuilder,
    app: &AppContext,
) -> TuiFlex {
    let header_style = builder.primary_text_style().add_modifier(Modifier::BOLD);
    let muted = builder.muted_text_style();
    let check = builder.success_glyph_style();

    let cwd_path = LocalOrRemotePath::Local(PathBuf::from(cwd));
    let rules = ProjectContextModel::as_ref(app).find_applicable_project_rules(&cwd_path);

    // Rule files that actively apply to the cwd, deduplicated by file name
    // (nested roots can contribute rules with the same name).
    let mut rule_files: Vec<String> = Vec::new();
    if let Some(rules) = &rules {
        for rule in &rules.active_rules {
            if let Some(name) = rule.path.file_name() {
                if !rule_files.iter().any(|file| file == name) {
                    rule_files.push(name.to_owned());
                }
            }
        }
    }

    let project_skill_count = SkillManager::as_ref(app)
        .get_skills_for_working_directory(Some(&cwd_path), app)
        .iter()
        .filter(|skill| skill.is_project_skill())
        .count();

    let header = rules
        .as_ref()
        .map(|rules| rules.root_path.display_path())
        .unwrap_or_else(|| cwd.to_owned());
    column = column.child(blank_row()).child(
        TuiText::new(abbreviate_home_prefix(&header))
            .with_style(header_style)
            .truncate()
            .finish(),
    );

    if rule_files.is_empty() && project_skill_count == 0 {
        // Repo detection, metadata indexing, and skill scans are async, so
        // nothing may be known yet; this also covers projects with no
        // context at all.
        return column.child(
            TuiText::new("Discovering project context…")
                .with_style(builder.dim_text_style())
                .truncate()
                .finish(),
        );
    }

    let status_row = |column: TuiFlex, text: String| {
        column.child(
            TuiFlex::row()
                .child(TuiText::new("✓ ").with_style(check).truncate().finish())
                .child(TuiText::new(text).with_style(muted).truncate().finish())
                .finish(),
        )
    };
    for file in rule_files {
        column = status_row(column, format!("{file} loaded"));
    }
    if project_skill_count > 0 {
        let plural = if project_skill_count == 1 { "" } else { "s" };
        column = status_row(
            column,
            format!("{project_skill_count} skill{plural} discovered"),
        );
    }
    column
}

/// A one-row spacer between sections.
fn blank_row() -> Box<dyn TuiElement> {
    TuiText::new(" ").truncate().finish()
}
