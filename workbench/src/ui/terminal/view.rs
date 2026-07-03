//! The terminal view: a focusable entity that owns the [`TermBackend`], routes
//! keyboard/mouse input to it, and runs a lightweight repaint tick so pty
//! output and the blinking cursor stay live without pinning a core.
//!
//! Rendering is delegated to [`TerminalElement`]; this view only holds state
//! (backend, focus, blink phase, last grid size) and the input handlers.

use std::time::Duration;

use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, ScrollDelta, ScrollWheelEvent, Styled, Task, Window,
};
use gpui_component::ActiveTheme;

use super::backend::{KeyInput, TermBackend, TermSnapshot};
use super::element::TerminalElement;

/// Blink half-period (ms). Rounded to the 16ms tick.
const BLINK_MS: u32 = 530;
const TICK_MS: u64 = 16;

pub struct TerminalView {
    backend: Option<TermBackend>,
    error: Option<String>,
    focus_handle: FocusHandle,
    blink_visible: bool,
    blink_accum: u32,
    last_size: (u16, u16),
    _tick: Task<()>,
}

impl TerminalView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (backend, error) = match TermBackend::spawn(None, 80, 24) {
            Ok(backend) => (Some(backend), None),
            Err(e) => (None, Some(format!("failed to start shell: {e}"))),
        };

        let mut view = Self {
            backend,
            error,
            focus_handle: cx.focus_handle(),
            blink_visible: true,
            blink_accum: 0,
            last_size: (80, 24),
            _tick: Task::ready(()),
        };
        view.schedule_tick(cx);
        view
    }

    /// Schedule the next repaint tick. Rescheduled from [`Self::on_tick`] so
    /// each firing owns the async context by value (the pattern gpui's own
    /// blink cursor uses), rather than looping over a borrowed one.
    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        self._tick = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(TICK_MS))
                .await;
            if let Some(this) = this.upgrade() {
                let _ = this.update(cx, |this, cx| this.on_tick(cx));
            }
        });
    }

    /// One tick: notify if the grid changed or the cursor blink phase flipped,
    /// then reschedule. Idle cost is a timer + an atomic load, not a re-render.
    fn on_tick(&mut self, cx: &mut Context<Self>) {
        let mut dirty = false;
        if let Some(backend) = self.backend.as_ref() {
            if backend.take_dirty() {
                dirty = true;
            }
        }
        self.blink_accum += TICK_MS as u32;
        if self.blink_accum >= BLINK_MS {
            self.blink_accum = 0;
            self.blink_visible = !self.blink_visible;
            dirty = true;
        }
        if dirty {
            cx.notify();
        }
        self.schedule_tick(cx);
    }

    /// Resize the backend to the visible grid (if changed) and snapshot it.
    /// Called by the element during layout.
    pub fn sync_and_snapshot(&mut self, cols: u16, rows: u16) -> Option<TermSnapshot> {
        let backend = self.backend.as_mut()?;
        if cols > 0 && rows > 0 && (cols, rows) != self.last_size {
            backend.resize(cols, rows);
            self.last_size = (cols, rows);
        }
        Some(backend.snapshot())
    }

    /// Whether this terminal currently holds keyboard focus.
    pub fn focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    /// Current cursor blink phase (visible half).
    pub fn blink(&self) -> bool {
        self.blink_visible
    }

    /// A clone of the focus handle, for the workspace to focus this terminal.
    pub fn handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(backend) = self.backend.as_mut() else {
            return;
        };
        let ks = &event.keystroke;
        let input = KeyInput {
            key: ks.key.clone(),
            key_char: ks.key_char.clone(),
            ctrl: ks.modifiers.control,
            alt: ks.modifiers.alt,
            shift: ks.modifiers.shift,
            platform: ks.modifiers.platform,
        };
        backend.send_input(&input);
        // Keep the cursor solid while typing.
        self.blink_visible = true;
        cx.notify();
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(backend) = self.backend.as_mut() else {
            return;
        };
        let dy = match event.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => p.y.as_f32() / 20.0,
        };
        if dy != 0.0 {
            // Wheel down (dy < 0) scrolls toward the live bottom.
            backend.on_scroll(dy < 0.0);
            cx.notify();
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let base = div()
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().background)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event, window, cx| {
                    window.focus(&this.focus_handle, cx);
                }),
            );

        if let Some(error) = &self.error {
            base.p_3()
                .text_color(cx.theme().danger)
                .child(error.clone())
        } else {
            base.child(TerminalElement::new(cx.entity()))
        }
    }
}
