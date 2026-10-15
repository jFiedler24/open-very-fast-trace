//! Zed extension shim: spawns `cargo-ovft ovft --lsp` as a language server.
//!
//! Requires `cargo-ovft` on the PATH (`cargo install cargo-ovft`).

use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

struct OvftExtension;

impl zed::Extension for OvftExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree).ok();
        let custom = settings.as_ref().and_then(|s| s.binary.as_ref());

        let command = custom
            .and_then(|b| b.path.clone())
            .or_else(|| worktree.which("cargo-ovft"))
            .ok_or_else(|| {
                "cargo-ovft not found in PATH. Install it with `cargo install cargo-ovft`."
                    .to_string()
            })?;

        let args = custom
            .and_then(|b| b.arguments.clone())
            .unwrap_or_else(|| vec!["ovft".to_string(), "--lsp".to_string()]);

        Ok(zed::Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(OvftExtension);
