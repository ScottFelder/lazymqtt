//! Built-in plugins compiled into the binary.

mod alerts;
mod analytics;
mod generator;
mod json_view;
mod observer;
mod recorder;
mod schema;
mod templates;
mod xml_view;

use super::Plugin;

/// Every built-in plugin, in dispatch order.
pub fn all() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(observer::JsonMarker::default()),
        Box::new(json_view::JsonView),
        Box::new(xml_view::XmlView),
        Box::new(alerts::TopicAlerts::default()),
        Box::new(schema::SchemaValidator::default()),
        Box::new(templates::PublishTemplates::default()),
        Box::new(generator::PayloadGenerator::default()),
        Box::new(analytics::TrafficAnalytics::default()),
        Box::new(recorder::Recorder::default()),
    ]
}
