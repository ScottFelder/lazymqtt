//! Built-in plugins compiled into the binary.

mod json_view;
mod observer;

use super::Plugin;

/// Every built-in plugin, in dispatch order.
pub fn all() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(observer::JsonMarker::default()),
        Box::new(json_view::JsonView),
    ]
}
