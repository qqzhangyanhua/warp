use std::path::Path;
use std::sync::Arc;

use pathfinder_geometry::vector::vec2f;
use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::watcher::DirectoryWatcher;
#[cfg(feature = "local_fs")]
use repo_metadata::RepoMetadataModel;
use string_offset::CharOffset;
use warp_core::features::FeatureFlag;
use warp_core::ui::appearance::Appearance;
use warp_editor::render::model::BlockItem;
#[cfg(feature = "local_fs")]
use warp_files::FileModel;
use warpui::platform::WindowStyle;
use warpui::{App, SingletonEntity, View};

use super::{FileNotebookView, FileState, MarkdownDisplayMode, SourceFile};
use crate::auth::auth_manager::AuthManager;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::CloudModel;
use crate::notebooks::context_menu::MenuSource;
use crate::notebooks::editor::keys::NotebookKeybindings;
use crate::notebooks::file::is_markdown_file;
use crate::search::files::model::FileSearchModel;
use crate::server::server_api::team::MockTeamClient;
use crate::server::server_api::workspace::MockWorkspaceClient;
use crate::server::server_api::ServerApiProvider;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::terminal::keys::TerminalKeybindings;
use crate::terminal::model::session::Session;
use crate::test_util::settings::initialize_settings_for_tests;
use crate::workspace::ActiveSession;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::{GlobalResourceHandles, GlobalResourceHandlesProvider};

fn init_app(app: &mut App) {
    initialize_settings_for_tests(app);

    let global_resource_handles = GlobalResourceHandles::mock(app);
    app.add_singleton_model(|_| GlobalResourceHandlesProvider::new(global_resource_handles));
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(|_| ActiveSession::default());
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(|_| DetectedRepositories::default());
    #[cfg(feature = "local_fs")]
    app.add_singleton_model(RepoMetadataModel::new);
    app.add_singleton_model(FileSearchModel::new);
    app.add_singleton_model(FileModel::new);
    app.add_singleton_model(NotebookKeybindings::new);
    app.add_singleton_model(TerminalKeybindings::new);
    app.add_singleton_model(CloudModel::mock);
    app.add_singleton_model(|_| ServerApiProvider::new_for_test());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
    app.add_singleton_model(AuthManager::new_for_test);
    let team_client_mock = Arc::new(MockTeamClient::new());
    let workspace_client_mock = Arc::new(MockWorkspaceClient::new());
    app.add_singleton_model(|ctx| {
        UserWorkspaces::mock(
            team_client_mock.clone(),
            workspace_client_mock.clone(),
            vec![],
            ctx,
        )
    });
    #[cfg(feature = "voice_input")]
    app.add_singleton_model(voice_input::VoiceInput::new);
}

#[test]
fn test_load_local() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);
        let session = Arc::new(Session::test());
        handle
            .update(&mut app, |file_notebook, ctx| {
                file_notebook.open_local("../README.md", Some(session), ctx);

                let file_id = file_notebook
                    .file_id
                    .expect("File should be opened and have a file_id");

                let future_handle = FileModel::as_ref(ctx)
                    .get_future_handle(file_id)
                    .expect("Loading future should be present");

                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        app.read(|ctx| {
            assert_eq!(&handle.as_ref(ctx).title(), "README.md");
            let location = handle
                .as_ref(ctx)
                .location
                .as_ref()
                .expect("Location should be set");
            assert_eq!(location.breadcrumbs, "..");

            let editor = handle.as_ref(ctx).editor.as_ref(ctx);
            // Local Markdown Notebooks are editable; owner/sharing/cloud actions remain absent.
            assert!(editor.is_editable(ctx));
            // We don't want to check the actual README contents, but it should be clearly non-empty.
            assert!(editor.markdown(ctx).len() > 4);
            assert!(!handle.as_ref(ctx).is_unsaved());
            assert!(!handle.as_ref(ctx).has_conflict());

            // Rendering should not panic.
            handle.as_ref(ctx).render(ctx);
        });
    });
}

#[cfg(feature = "local_fs")]
#[test]
fn test_new_unsaved_notebook_until_first_save() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);

        handle.update(&mut app, |file_notebook, ctx| {
            file_notebook.open_unsaved(Some("My Notes".into()), Some("# draft\n".into()), ctx);
        });

        app.read(|ctx| {
            let view = handle.as_ref(ctx);
            assert!(view.is_unsaved());
            assert!(view.local_path().is_none());
            // Unsaved has no path; dirty marker only applies after mark_edited on Bound.
            assert_eq!(view.title(), "My Notes");
            assert!(view.editor.as_ref(ctx).is_editable(ctx));
            assert!(view.editor.as_ref(ctx).markdown(ctx).contains("draft"));
            // Relative content has no base until the Notebook is bound to a path.
            assert!(view
                .local_session
                .as_ref()
                .and_then(|s| s.document_path_for_relative_content())
                .is_none());
            // No cloud ID, owner, or sharing surface on the local file view.
            view.render(ctx);
        });

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("my_notes.md");
        handle.update(&mut app, |file_notebook, ctx| {
            file_notebook.finish_save_as(path.clone(), ctx);
        });

        // first_save writes then re-opens via FileModel; wait for load.
        handle
            .update(&mut app, |file_notebook, ctx| {
                let file_id = file_notebook
                    .file_id
                    .expect("File should be opened after first save");
                let future_handle = FileModel::as_ref(ctx)
                    .get_future_handle(file_id)
                    .expect("Loading future should be present");
                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        app.read(|ctx| {
            let view = handle.as_ref(ctx);
            assert!(!view.is_unsaved());
            // FileModel may canonicalize paths (e.g. /var vs /private/var on macOS).
            let opened = view.local_path().expect("path after first save");
            assert_eq!(opened.file_name(), path.file_name());
            assert!(std::fs::read_to_string(&path).unwrap().contains("draft"));
            assert!(!view.has_conflict());
            // After bind, session exposes the file path for relative content resolution.
            let doc = view
                .local_session
                .as_ref()
                .and_then(|s| s.document_path_for_relative_content())
                .expect("bound notebook has document path");
            assert_eq!(doc.file_name(), path.file_name());
        });
    });
}

#[cfg(feature = "local_fs")]
#[test]
fn test_cancel_first_save_keeps_unsaved() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);

        handle.update(&mut app, |file_notebook, ctx| {
            file_notebook.open_unsaved(Some("Draft".into()), Some("body".into()), ctx);
        });

        app.read(|ctx| {
            assert!(handle.as_ref(ctx).is_unsaved());
            assert!(handle.as_ref(ctx).local_path().is_none());
            assert!(handle.as_ref(ctx).editor.as_ref(ctx).markdown(ctx).contains("body"));
        });
    });
}

#[cfg(feature = "local_fs")]
#[test]
fn test_external_conflict_surfaces_visible_state() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conflict.md");
        std::fs::write(&path, "original\n").unwrap();

        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);
        let session = Arc::new(Session::test());
        handle
            .update(&mut app, |file_notebook, ctx| {
                file_notebook.open_local(&path, Some(session), ctx);
                let file_id = file_notebook.file_id.expect("file id");
                let future_handle = FileModel::as_ref(ctx)
                    .get_future_handle(file_id)
                    .expect("load future");
                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        handle.update(&mut app, |file_notebook, ctx| {
            // Simulate a local edit then external conflict via the session seam.
            file_notebook.editor.update(ctx, |editor, ctx| {
                editor.reset_with_markdown("local edit\n", ctx);
            });
            if let Some(session) = file_notebook.local_session.as_mut() {
                session.mark_edited();
                session.apply_save_conflict();
            }
            ctx.notify();
        });

        app.read(|ctx| {
            let view = handle.as_ref(ctx);
            // Conflict supersedes Dirty in the session enum.
            assert!(view.has_conflict());
            assert!(!view.is_dirty());
            // Conflict banner must not panic.
            view.render(ctx);
        });
    });
}

#[test]
fn test_load_jupyter_notebook_renders_cells() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let _flag = FeatureFlag::JupyterNotebookRendering.override_enabled(true);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("analysis.ipynb");
        std::fs::write(
            &path,
            r##"{
                "nbformat": 4,
                "nbformat_minor": 5,
                "metadata": {"language_info": {"name": "python"}},
                "cells": [
                    {"cell_type": "markdown", "source": ["# Notebook heading"]},
                    {"cell_type": "code", "source": "print('hello')", "outputs": []}
                ]
            }"##,
        )
        .unwrap();

        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);
        let session = Arc::new(Session::test());
        handle
            .update(&mut app, |file_notebook, ctx| {
                file_notebook.open_local(&path, Some(session), ctx);

                let file_id = file_notebook
                    .file_id
                    .expect("File should be opened and have a file_id");

                let future_handle = FileModel::as_ref(ctx)
                    .get_future_handle(file_id)
                    .expect("Loading future should be present");

                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        app.read(|ctx| {
            let editor = handle.as_ref(ctx).editor.as_ref(ctx);
            let markdown = editor.markdown(ctx);
            // The notebook is rendered (heading from the markdown cell shows),
            // and the raw JSON is not (no `nbformat` key leaks through).
            assert!(
                markdown.contains("Notebook heading"),
                "expected rendered heading, got: {markdown}"
            );
            assert!(
                !markdown.contains("nbformat"),
                "raw notebook JSON should not be shown, got: {markdown}"
            );

            // The Rendered/Raw toggle is exposed for .ipynb, the same way it is
            // for markdown files (PRODUCT invariant 14).
            assert!(
                handle.as_ref(ctx).shows_markdown_toggle(),
                "rendered notebook should expose the Rendered/Raw toggle"
            );

            // Rendering should not panic.
            handle.as_ref(ctx).render(ctx);
        });
    });
}

#[test]
fn test_malformed_jupyter_notebook_falls_back_to_raw() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let _flag = FeatureFlag::JupyterNotebookRendering.override_enabled(true);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.ipynb");
        // Invalid notebook JSON that also contains Markdown which must NOT be
        // rendered as Markdown (PRODUCT invariant 11: fall back to raw text).
        std::fs::write(&path, "{ \"nbformat\": 4, broken json # Heading").unwrap();

        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);
        let session = Arc::new(Session::test());
        handle
            .update(&mut app, |file_notebook, ctx| {
                file_notebook.open_local(&path, Some(session), ctx);

                let file_id = file_notebook
                    .file_id
                    .expect("File should be opened and have a file_id");

                let future_handle = FileModel::as_ref(ctx)
                    .get_future_handle(file_id)
                    .expect("Loading future should be present");

                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        app.read(|ctx| {
            let editor = handle.as_ref(ctx).editor.as_ref(ctx);
            let markdown = editor.markdown(ctx);
            // The raw contents are shown verbatim (never a blank view), fenced
            // as a code block rather than interpreted as Markdown.
            assert!(
                markdown.contains("broken json"),
                "expected raw contents shown, got: {markdown}"
            );
            assert!(
                markdown.contains("```"),
                "raw fallback should be fenced, got: {markdown}"
            );

            // Rendering should not panic.
            handle.as_ref(ctx).render(ctx);
        });
    });
}

#[test]
fn test_load_before_session() {
    // There might not be a session if:
    // * Restoring a file notebook, since terminal panes won't have bootstrapped yet
    // * Only notebooks are open
    App::test((), |mut app| async move {
        init_app(&mut app);
        let (window_id, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);

        // Open a file we know exists to verify that the view can render.
        handle
            .update(&mut app, |file_notebook, ctx| {
                file_notebook.open_local("../README.md", None, ctx);
                match &file_notebook.file_state {
                    FileState::Loading(SourceFile::FileBased { path, .. }) => {
                        assert_eq!(path.to_local_path(), Some(Path::new("../README.md")))
                    }
                    other => panic!("Expected FileState::Loading(FileBased), got {other:?}"),
                }

                let file_id = file_notebook
                    .file_id
                    .expect("File should be opened and have a file_id");

                let future_handle = FileModel::as_ref(ctx)
                    .get_future_handle(file_id)
                    .expect("Loading future should be present");

                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        handle.read(&app, |view, _| {
            let expected_path = dunce::canonicalize("../README.md").expect("Path exists");

            assert_eq!(view.title(), expected_path.display().to_string());
            assert!(view.location.is_none());

            match &view.file_state {
                FileState::Loaded(SourceFile::FileBased { path, .. }) => {
                    assert_eq!(path.to_local_path(), Some(expected_path.as_path()));
                }
                other => panic!("Expected FileState::Loaded(FileBased), got {other:?}"),
            };
        });

        // Once a local session is available, the view should use it.
        let session = Arc::new(Session::test());
        ActiveSession::handle(&app).update(&mut app, |active_session, ctx| {
            active_session.set_session_for_test(window_id, session.clone(), Some("."), None, ctx);
        });

        handle.read(&app, |view, _| {
            assert_eq!(&view.title(), "README.md");
            // The location should be set, but the exact breadcrumbs depend on where the repo
            // is located.
            assert!(view.location.is_some());
        });
    });
}

#[test]
fn test_load_static() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);

        handle.update(&mut app, |file_notebook, ctx| {
            file_notebook.open_static("Test Title", "Test Content", ctx);
            assert!(file_notebook.file_id.is_none());

            assert!(matches!(file_notebook.file_state, FileState::Loaded(_)));
            assert_eq!(file_notebook.title(), "Test Title");
            assert!(file_notebook.location.is_none());

            let editor = file_notebook.editor.as_ref(ctx);
            assert!(!editor.is_editable(ctx));
            // We don't want to check the actual README contents, but it should be clearly non-empty.
            assert!(editor.markdown(ctx).len() > 4);

            // Rendering should not panic.
            file_notebook.render(ctx);
        });
    });
}

#[test]
fn test_file_notebook_mermaid_blocks_default_to_rendered() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let _flag = FeatureFlag::MarkdownMermaid.override_enabled(true);
        let _editable_flag = FeatureFlag::EditableMarkdownMermaid.override_enabled(true);
        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);

        handle.update(&mut app, |file_notebook, ctx| {
            file_notebook.open_static("Test Title", "```mermaid\ngraph TD\nA --> B\n```", ctx);
        });
        let render_state = handle.read(&app, |view, ctx| {
            view.editor
                .as_ref(ctx)
                .model()
                .as_ref(ctx)
                .render_state()
                .clone()
        });
        app.read(|ctx| render_state.as_ref(ctx).layout_complete())
            .await;
        app.read(|ctx| render_state.as_ref(ctx).layout_complete())
            .await;

        handle.read(&app, |view, ctx| {
            let editor = view.editor.as_ref(ctx);
            let model = editor.model().as_ref(ctx);
            let command = model
                .notebook_command_for_block(CharOffset::zero())
                .expect("Mermaid command should exist");
            assert_eq!(
                command.as_ref(ctx).mermaid_display_mode,
                MarkdownDisplayMode::Rendered
            );
            assert!(matches!(
                model
                    .render_state()
                    .as_ref(ctx)
                    .content()
                    .block_at_height(0.)
                    .map(|item| item.item),
                Some(BlockItem::MermaidDiagram { .. })
            ));
        });
    });
}

#[test]
fn test_markdown_file_detection() {
    assert!(is_markdown_file("README.md"));
    assert!(is_markdown_file("DATABASE.MD"));
    assert!(is_markdown_file("notes.markdown"));
    assert!(is_markdown_file("README"));
    assert!(is_markdown_file("license"));
    assert!(is_markdown_file("CHANGELOG"));
    assert!(is_markdown_file("ReadMe"));

    assert!(!is_markdown_file("README.txt"));
    assert!(!is_markdown_file("main.rs"));
    assert!(!is_markdown_file("notes"));
}

#[test]
fn test_file_notebook_mermaid_context_menu_does_not_show_copy_image() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        let (_, handle) = app.add_window(WindowStyle::NotStealFocus, FileNotebookView::new);

        handle.update(&mut app, |file_notebook, ctx| {
            file_notebook.open_static("Test Title", "```mermaid\ngraph TD\nA --> B\n```", ctx);

            let source = MenuSource::RichTextEditor {
                parent_offset: vec2f(0., 0.),
                editor: file_notebook.editor.clone(),
            };
            file_notebook.context_menu.show_context_menu(source, ctx);

            let item_names = file_notebook.context_menu.item_names(ctx);
            assert!(!item_names.contains(&"Copy image"));
        });
    });
}
