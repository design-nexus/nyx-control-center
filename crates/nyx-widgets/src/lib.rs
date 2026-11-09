//! The Nyx widget framework, shared by the Control Center and (later) desktop widgets.
//!
//! The rule it keeps: **what a widget knows is apart from how it's drawn.** A provider
//! (Wi-Fi, volume, weather) publishes its state as a [`live::Live`] value; a view
//! subscribes while it's on screen and draws it however its host wants (a 1×1 tile in
//! the Control Center, an expanded card on the desktop). The provider runs only while
//! something is subscribed, so a hidden panel costs nothing.
//!
//! - [`live`]: a value with subscribers that starts and stops its source.
//! - [`layout`]: tile sizes and pages of a 4-column grid, packed first-fit, as saved in
//!   the user's layout file.
//! - [`settings`]: that file through NyxOS's configuration layers.

pub mod layout;
pub mod live;
pub mod settings;
