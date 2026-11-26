//! The name prompt that covers Play until a player is chosen. It lives where Play was pressed:
//! no trip to Accounts, and nothing starts until a valid name is confirmed.
use super::*;
use crate::glass_ui::{self as ui, white};
use horizon_theme::glass::{ink, radius};

/// The game waiting behind the prompt, and the field's state.
pub(super) struct AskName {
    pub(super) tag: String,
    pub(super) input: line_edit::LineEdit,
    pub(super) focus: Option<FocusHandle>,
    pub(super) focused: bool,
    pub(super) error: Option<String>,
    /// Focus the field on the next frame, once its handle exists.
    pub(super) focus_soon: bool,
}

impl AskName {
    pub(super) fn new(tag: String, suggested: String) -> Self {
        let mut input = line_edit::LineEdit::new(suggested, 16, 24);
        input.select_all();
        Self {
            tag,
            input,
            focus: None,
            focused: false,
            error: None,
            focus_soon: true,
        }
    }
}

impl Launcher {
    /// The prompt's focus is made on the first frame, where the window is available.
    pub(super) fn note_ask_name_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.ask_name.as_mut() else {
            return;
        };
        let focus = ask
            .focus
            .get_or_insert_with(|| cx.focus_handle().tab_index(0).tab_stop(true));
        if ask.focus_soon {
            ask.focus_soon = false;
            window.focus(focus, cx);
        }
        ask.focused = focus.is_focused(window);
    }

    pub(super) fn ask_name_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.ask_name.is_none() {
            return false;
        }
        self.cancel_ask_name(window, cx);
        true
    }

    pub(super) fn cancel_ask_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ask_name = None;
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    /// Saves the name as the chosen offline player, then starts the game that was waiting.
    pub(super) fn confirm_ask_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.ask_name.as_ref() else {
            return;
        };
        let name = ask.input.text().trim().to_string();
        let tag = ask.tag.clone();
        if !account::valid_offline_name(&name) {
            if let Some(ask) = self.ask_name.as_mut() {
                ask.error = Some(tr("Use 3 to 16 letters, numbers or _.").into());
            }
            cx.notify();
            return;
        }

        let mut accounts = Accounts::load();
        let done = accounts
            .play_as_with(
                account::store(),
                account::Player::Offline(name.clone()),
                self.settings.dev_offline,
            )
            .and_then(|()| accounts.save())
            .and_then(|()| instances::write_player_name(&name));
        match done {
            Ok(()) => {
                self.accounts = accounts;
                self.offline_name = Some(name);
                self.ask_name = None;
                window.focus(&self.root_focus, cx);
                self.start_client(tag);
            }
            Err(e) => {
                if let Some(ask) = self.ask_name.as_mut() {
                    ask.error = Some(e);
                }
                cx.notify();
            }
        }
    }

    /// The modal, over the page the player was looking at.
    pub(super) fn ask_name_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let ask = self.ask_name.as_ref()?;
        let focus = ask.focus.as_ref()?;
        let t = self.theme;
        let focused = ask.focused;
        let field = div()
            .id("ask-name-input")
            .track_focus(focus)
            .flex()
            .items_center()
            .h(px(48.))
            .px(px(14.))
            .rounded(px(radius::ROW))
            .bg(white(if focused { 0.11 } else { 0.07 }))
            .font_family(horizon_theme::glass::FONT)
            .text_size(px(15.))
            .cursor_text()
            .overflow_hidden()
            .when(focused, |d| d.child(ui::focus_ring(radius::ROW)))
            .when_some(ask.error.as_ref(), |d, _| {
                d.border_1().border_color(ui::color(horizon_theme::glass::DANGER))
            })
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(line_edit::content(
                        &ask.input,
                        &t,
                        focused,
                        false,
                        false,
                        "Your offline name",
                    )),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                if let Some(focus) = this.ask_name.as_ref().and_then(|a| a.focus.as_ref()) {
                    window.focus(focus, cx);
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let Some(ask) = this.ask_name.as_mut() else {
                    return;
                };
                let key = line_edit::handle(&mut ask.input, ev, cx);
                match key {
                    line_edit::Key::Pass => return,
                    line_edit::Key::Enter => this.confirm_ask_name(window, cx),
                    line_edit::Key::Escape => this.cancel_ask_name(window, cx),
                    key => {
                        if matches!(key, line_edit::Key::Edited | line_edit::Key::Paste | line_edit::Key::Cut(_)) {
                            if let Some(ask) = this.ask_name.as_mut() {
                                ask.error = None;
                            }
                        }
                    }
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let field = line_edit::mouse(field, cx, |this: &mut Launcher| {
            &mut this.ask_name.as_mut().unwrap().input
        });

        let card = ui::dialog_plate(radius::MODAL)
            .id("ask-name-card")
            .occlude()
            .flex()
            .flex_col()
            .w(px(420.))
            .max_w_full()
            .p(px(28.))
            .child(ui::text(13., FontWeight::MEDIUM, ink::QUIET).child(tr("Offline player")))
            .child(
                div()
                    .mt(px(8.))
                    .font_family(horizon_theme::glass::DISPLAY_FONT)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(26.))
                    .line_height(px(33.))
                    .text_color(white(ink::FULL))
                    .child(tr("Choose your name")),
            )
            .child(
                div()
                    .mt(px(10.))
                    .mb(px(20.))
                    .child(ui::text(14., FontWeight::NORMAL, ink::BODY).child(tr(
                        "Horizon plays offline. Pick the name other players will see.",
                    ))),
            )
            .child(field)
            .child(
                div()
                    .mt(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when_some(ask.error.as_ref(), |col, error| {
                        col.child(
                            div()
                                .font_family(horizon_theme::glass::FONT)
                                .text_size(px(12.))
                                .text_color(ui::color(horizon_theme::glass::DANGER))
                                .child(error.to_string()),
                        )
                    })
                    .child(ui::text(12., FontWeight::NORMAL, ink::NOTE).child(tr(
                        "3 to 16 letters, numbers or _. You can change it later in Settings → Account.",
                    ))),
            )
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        ui::button("ask-name-cancel", tr("Cancel"), Kind::Ghost, &t).on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.cancel_ask_name(window, cx);
                            }),
                        ),
                    )
                    .child(
                        ui::button("ask-name-play", tr("Play"), Kind::Primary, &t).on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.confirm_ask_name(window, cx);
                            }),
                        ),
                    ),
            );

        Some(
            div()
                .id("ask-name-dialog")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .occlude()
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(ui::scrim_backdrop())
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(ui::scrim())
                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _: &gpui::MouseDownEvent, window, cx| {
                            this.cancel_ask_name(window, cx);
                        })),
                )
                .child(card),
        )
    }
}
