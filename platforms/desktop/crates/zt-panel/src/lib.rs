//! ZhuyinType's candidate window without a UI toolkit.
//!
//! What the window shows ([`model`]), how the settings shape it ([`style`]), where everything goes ([`layout`]), its
//! colours and sizes ([`tokens`]), how keys are written ([`keys`]) and where the window goes on screen ([`position`]).
//! The GPUI app draws it (Linux, the previews) and so does the Windows text service, inside the application being
//! typed in; both use this crate, so the two windows have the same geometry and colours.

pub mod keys;
pub mod layout;
pub mod model;
pub mod position;
pub mod style;
pub mod tokens;
