//! The candidate window: the element that draws a `zt_core::presentation::Presentation` exactly as the macOS window
//! does (`panel`, laid out by `layout` from the 「視窗與提示」 settings in `style`), the floating windows that show it
//! beside the caret (`popup`, with the platform shims in `platform`), and the screenshot showcase.

pub mod layout;
pub mod panel;
pub mod picker;
pub mod platform;
pub mod popup;
pub mod preview;
pub mod quick;
pub mod sample;
pub mod scene;
pub mod showcase;
pub mod style;
