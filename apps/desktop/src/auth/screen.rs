//! Full-window egui screens rendered in the unauthenticated / loading states.
//!
//! - `LoadingScreen` — shown during the cold-start keychain restore window
//!   (~50-500ms). Non-interactive so the user cannot click "Sign In" and
//!   accidentally open an OAuth flow while a keychain refresh is in flight.
//! - `LoginScreen` — shown when unauthenticated or when a sign-in flow is
//!   active (OAuth browser wait, failure + retry).

use eframe::egui;

use super::config::ClerkConfig;
use super::signin;
use super::state::{AuthState, AuthStateHandle};

/// Non-interactive full-window placeholder shown while a keychain refresh is
/// in flight at cold start. Prevents the user from clicking "Sign In" and
/// opening an unwanted OAuth browser flow during the ~50-500ms restore window.
#[derive(Default)]
pub struct LoadingScreen;

impl LoadingScreen {
    pub fn show(&self, ctx: &egui::Context) {
        #[allow(deprecated)]
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::BG))
            .show(ctx, |ui| {
                ui.with_layout(
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add(egui::Spinner::new().size(24.0));
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("Restoring session\u{2026}")
                                    .size(13.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        });
                    },
                );
            });

        // Request repaint so the spinner animates during the restore window.
        ctx.request_repaint_after(std::time::Duration::from_millis(120));
    }
}

pub struct LoginScreen {
    cfg: ClerkConfig,
    state: AuthStateHandle,
    runtime: tokio::runtime::Handle,
}

impl LoginScreen {
    pub fn new(cfg: ClerkConfig, state: AuthStateHandle, runtime: tokio::runtime::Handle) -> Self {
        Self {
            cfg,
            state,
            runtime,
        }
    }

    /// Render the screen. Returns `true` if the user just transitioned to
    /// `Authenticated` (caller can stop rendering this screen and switch
    /// to the main UI). Otherwise returns `false`.
    pub fn show(&self, ctx: &egui::Context) -> bool {
        let snapshot = self.state.blocking_snapshot();
        if matches!(snapshot, AuthState::Authenticated { .. }) {
            return true;
        }

        #[allow(deprecated)]
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.heading(egui::RichText::new("Zaned").size(48.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Sign in to continue").size(16.0).weak());
                ui.add_space(48.0);

                match snapshot {
                    AuthState::Unauthenticated => {
                        if ui
                            .add_sized([240.0, 44.0], egui::Button::new("Sign in with Clerk"))
                            .clicked()
                        {
                            let cfg = self.cfg.clone();
                            let state = self.state.clone();
                            let join = self.runtime.spawn(async move {
                                signin::run(cfg, state).await;
                            });
                            // Register the abort handle so Cancel can stop this task.
                            let abort = join.abort_handle();
                            let state2 = self.state.clone();
                            self.runtime.spawn(async move {
                                state2.register_in_flight(abort).await;
                            });
                        }
                    }
                    AuthState::Loading => {
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label("Waiting for browser sign-in\u{2026}");
                        ui.add_space(8.0);
                        if ui
                            .add_sized([120.0, 32.0], egui::Button::new("Cancel"))
                            .clicked()
                        {
                            self.state.cancel_and_reset_from_sync();
                        }
                    }
                    AuthState::Failed { message } => {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            format!("Sign-in failed: {message}"),
                        );
                        ui.add_space(16.0);
                        if ui
                            .add_sized([160.0, 36.0], egui::Button::new("Try again"))
                            .clicked()
                        {
                            self.state.cancel_and_reset_from_sync();
                        }
                    }
                    AuthState::Authenticated { .. } => {
                        // Already handled above; unreachable in practice.
                    }
                }
            });
        });

        // Repaint while we're waiting on a flow so the spinner animates.
        ctx.request_repaint_after(std::time::Duration::from_millis(120));
        false
    }
}
