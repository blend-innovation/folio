//! In-app updates.
//!
//! Folio never phones home on its own: the updater runs only when the reader
//! chooses **Check for Updates…** from the Folio menu. The release manifest
//! points at a GitHub Release asset, and the artifact must match the public key
//! in `tauri.conf.json` (`plugins.updater.pubkey`) before anything is installed.
//! A build signed by anyone else is refused by the updater, not by us.

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

const TITLE: &str = "Software Update";

/// Runs the whole update conversation off the UI thread.
///
/// Two reasons: `check`/`download` are network calls that would otherwise sit on
/// a runtime worker while the user waits, and `blocking_show` is a native modal
/// that must not be shown from the main thread.
pub fn check(app: AppHandle) {
    std::thread::spawn(move || {
        tauri::async_runtime::block_on(converse(app));
    });
}

async fn converse(app: AppHandle) {
    let current = app.package_info().version.to_string();

    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(error) => {
            return say(
                &app,
                MessageDialogKind::Error,
                format!("Folio cannot check for updates.\n\n{error}"),
            )
        }
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            return say(
                &app,
                MessageDialogKind::Info,
                format!("Folio {current} is the newest version."),
            )
        }
        Err(error) => {
            return say(
                &app,
                MessageDialogKind::Error,
                format!("Folio could not reach the update server.\n\n{error}"),
            )
        }
    };

    // Installing from a `cargo` debug build would replace the bundle the
    // developer is running, so debug builds stop after reporting the version.
    if cfg!(debug_assertions) {
        return say(
            &app,
            MessageDialogKind::Info,
            format!(
                "Folio {} is available. You are running a development build ({}), so nothing was installed.",
                update.version, current
            ),
        );
    }

    let mut body = format!(
        "Folio {} is available. You have {}.",
        update.version, current
    );
    if let Some(notes) = update.body.as_deref().map(str::trim) {
        if !notes.is_empty() {
            body.push_str("\n\n");
            body.push_str(notes);
        }
    }
    body.push_str("\n\nInstall it now? Folio reopens when the download finishes.");

    let install = app
        .dialog()
        .message(body)
        .title(TITLE)
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Install".into(),
            "Later".into(),
        ))
        .blocking_show();

    if !install {
        return;
    }

    // The download is verified against the signing key first; only then does
    // the updater replace the bundle in place.
    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(()) => app.restart(),
        Err(error) => say(
            &app,
            MessageDialogKind::Error,
            format!("The update could not be installed.\n\n{error}"),
        ),
    }
}

fn say(app: &AppHandle, kind: MessageDialogKind, body: impl Into<String>) {
    app.dialog()
        .message(body.into())
        .title(TITLE)
        .kind(kind)
        .blocking_show();
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    /// The feed is compiled into the app, so a config edit is a release change.
    /// Catch the two ways it breaks quietly: no key (nothing verifies) and a
    /// plaintext endpoint (refused in release builds).
    #[test]
    fn the_update_feed_is_signed_and_encrypted() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &config["plugins"]["updater"];

        let pubkey = updater["pubkey"].as_str().expect("plugins.updater.pubkey");
        let decoded = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(pubkey.trim())
                .expect("pubkey is base64"),
        )
        .expect("pubkey is text");
        assert!(decoded.contains("minisign public key"), "{decoded}");

        let endpoints = updater["endpoints"].as_array().expect("endpoints");
        assert!(!endpoints.is_empty());
        for endpoint in endpoints {
            let url = endpoint.as_str().expect("endpoint is a string");
            assert!(url.starts_with("https://"), "{url}");
        }
    }
}
