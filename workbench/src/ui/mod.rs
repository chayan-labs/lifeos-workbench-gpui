//! GPU-native gpui frontend.
//!
//! `gpui` (Zed's Metal UI engine) + `gpui-component` (native widget library)
//! render the Workbench as a real GUI: resizable panels, native menus, a
//! terminal with a real cursor, and mouse-first interaction throughout. The
//! shared logic core (command registry, layout, manifests, view-model
//! builders) is consumed by the views here; nothing gpui leaks back into it.

pub mod app;
pub mod workspace_view;

pub use app::run;
