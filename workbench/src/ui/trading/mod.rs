//! The Trading pane: a read-only broker positions readout.
//!
//! `GET /api/broker/positions` is the *only* market-data route `lifeos-api`
//! exposes for trading (confirmed by the route audit: no place/modify/
//! cancel/GTT route exists anywhere on the router) - so this pane is
//! permanently read-only, not a policy toggle that could later be flipped.
//! The `journal`/`setups`/`proposed_order` entities the trading module's own
//! manifest declares already render correctly through the Life OS pane's
//! `table`/`calendar` view-kind dispatch (Part 4), so this pane doesn't
//! duplicate that renderer - it's positions plus a shortcut into Life OS.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, StyledExt};
use serde_json::Value;

use super::actions::OpenLifeOs;
use super::api_host::{ApiHost, HostStatus};
use super::theme::pane_bg;

const POLL_MS: u64 = 150;

#[derive(Clone, Debug)]
struct Position {
    symbol: String,
    quantity: f64,
    average_price: f64,
    pnl: f64,
}

#[derive(Default)]
struct State {
    busy: bool,
    error: Option<String>,
    positions: Vec<Position>,
    fetched: bool,
}

pub struct TradingView {
    api: ApiHost,
    state: Arc<Mutex<State>>,
    focus: FocusHandle,
    _poll: Task<()>,
}

impl TradingView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = Self {
            api,
            state: Arc::default(),
            focus: cx.focus_handle(),
            _poll: Task::ready(()),
        };
        view.refresh(cx);
        view
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn refresh(&self, cx: &mut Context<Self>) {
        let (api, token) = match self.api.status() {
            HostStatus::Ready(api, token) => (api, token),
            HostStatus::Booting => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some("connecting to lifeos-api...".into());
                }
                return;
            }
            HostStatus::Failed(e) => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some(e);
                }
                return;
            }
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get("/api/broker/positions", token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                s.fetched = true;
                if response.is_success() {
                    s.positions = parse_positions(&response.body);
                    s.error = None;
                } else if response.status.as_u16() == 501 {
                    s.error = Some(
                        "Kite is not configured - no broker connection for this workspace".into(),
                    );
                } else {
                    s.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn start_poll(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    let busy = this.state.lock().map(|s| s.busy).unwrap_or(false);
                    cx.notify();
                    if busy {
                        this.start_poll(cx);
                    }
                });
            }
        })
        .detach();
    }
}

impl Focusable for TradingView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TradingView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let banner = div()
            .h_flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_3()
            .py_2()
            .bg(cx.theme().warning.opacity(0.15))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child("Read-only \u{2014} no order placement is possible from this app"),
            )
            .child(
                Button::new("open-lifeos-trading")
                    .label("Journal / setups in Life OS \u{2192}")
                    .small()
                    .ghost()
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenLifeOs), cx)),
            );

        let s = self.state.lock();
        let mut body = div()
            .id("trading-positions")
            .v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_2()
            .gap_1();
        match s.as_deref() {
            Ok(s) if s.busy && !s.fetched => body = body.child(hint("loading positions...", cx)),
            Ok(s) if s.error.is_some() => {
                body = body.child(hint(s.error.as_deref().unwrap_or(""), cx))
            }
            Ok(s) if s.positions.is_empty() => body = body.child(hint("no open positions", cx)),
            Ok(s) => {
                let header = div()
                    .h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().flex_1().child("Symbol"))
                    .child(div().w(gpui::px(80.0)).child("Qty"))
                    .child(div().w(gpui::px(100.0)).child("Avg price"))
                    .child(div().w(gpui::px(100.0)).child("P&L"));
                body = body.child(header).children(s.positions.iter().map(|p| {
                    let pnl_color = if p.pnl >= 0.0 {
                        cx.theme().success
                    } else {
                        cx.theme().danger
                    };
                    div()
                        .h_flex()
                        .w_full()
                        .px_2()
                        .py_1()
                        .gap_2()
                        .text_sm()
                        .child(
                            div()
                                .flex_1()
                                .text_color(cx.theme().foreground)
                                .child(p.symbol.clone()),
                        )
                        .child(div().w(gpui::px(80.0)).child(format!("{}", p.quantity)))
                        .child(
                            div()
                                .w(gpui::px(100.0))
                                .child(format!("{:.2}", p.average_price)),
                        )
                        .child(
                            div()
                                .w(gpui::px(100.0))
                                .text_color(pnl_color)
                                .child(format!("{:.2}", p.pnl)),
                        )
                }));
            }
            Err(_) => body = body.child(hint("state lock poisoned", cx)),
        }
        drop(s);

        div()
            .track_focus(&self.focus)
            .key_context("Trading")
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(banner)
            .child(body)
    }
}

fn hint(text: &str, cx: &Context<TradingView>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

/// Kite's `positions` payload nests day/net position arrays; this reads
/// `net` (falling back to `day`) since that's what a "what do I hold right
/// now" readout means.
fn parse_positions(body: &Value) -> Vec<Position> {
    let arr = body["net"]
        .as_array()
        .or_else(|| body["day"].as_array())
        .or_else(|| body.as_array());
    arr.map(|rows| {
        rows.iter()
            .map(|r| Position {
                symbol: r["tradingsymbol"].as_str().unwrap_or("?").to_string(),
                quantity: r["quantity"].as_f64().unwrap_or(0.0),
                average_price: r["average_price"].as_f64().unwrap_or(0.0),
                pnl: r["pnl"].as_f64().unwrap_or(0.0),
            })
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_net_positions_from_the_kite_shape() {
        let body = json!({
            "net": [{"tradingsymbol": "INFY", "quantity": 10, "average_price": 1500.0, "pnl": 250.5}],
            "day": [],
        });
        let positions = parse_positions(&body);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, "INFY");
        assert!((positions[0].pnl - 250.5).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_day_positions_when_net_is_absent() {
        let body = json!({ "day": [{"tradingsymbol": "TCS", "quantity": 5, "average_price": 3000.0, "pnl": -10.0}] });
        let positions = parse_positions(&body);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, "TCS");
    }
}
