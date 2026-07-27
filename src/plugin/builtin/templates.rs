//! Built-in publish-templates plugin (`publish-templates`).
//!
//! Exposes each saved template as a command in the `m` menu; invoking one opens
//! the publish form pre-filled (`PluginAction::OpenPublish`) so you can tweak
//! any `{{placeholder}}`s and review before sending. Templates are global and
//! stored in `plugins/publish-templates.json` — created in-app by "save as
//! template" from the publish form, or hand-edited.

use crate::plugin::api::{PluginAction, PluginCommand, PluginContext, PluginEvent, PluginMetadata};
use crate::plugin::{templates, Plugin};
use std::path::PathBuf;

const NAME: &str = "publish-templates";
const NONE_ID: &str = "__none__"; // placeholder command id when no templates exist

#[derive(Default)]
pub struct PublishTemplates {
    config_dir: PathBuf,
}

impl Plugin for PublishTemplates {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: NAME,
            version: "0.1.0",
            description: "saved publish presets, opened pre-filled from the m menu",
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        self.config_dir = ctx.config_dir.clone();
        Ok(())
    }

    fn on_event(&mut self, _event: &PluginEvent) -> Vec<PluginAction> {
        Vec::new()
    }

    /// One command per template, read fresh so newly-saved templates appear
    /// without a restart. The command id is the template name. When there are no
    /// templates yet, show a hint row so the enabled plugin is still visible and
    /// tells you where to add them.
    fn commands(&self) -> Vec<PluginCommand> {
        let templates = templates::load(&self.config_dir);
        if templates.is_empty() {
            return vec![PluginCommand::action(
                NONE_ID,
                "↥",
                "No templates — save one with ^T on the publish form",
            )];
        }
        templates
            .into_iter()
            .map(|t| PluginCommand::action(t.name.clone(), "↥", format!("Publish: {}", t.name)))
            .collect()
    }

    fn invoke(&mut self, id: &str) -> Vec<PluginAction> {
        if id == NONE_ID {
            return vec![PluginAction::Status(
                "add templates with ^T on the publish form, or edit plugins/publish-templates.json"
                    .into(),
            )];
        }
        match templates::load(&self.config_dir)
            .into_iter()
            .find(|t| t.name == id)
        {
            Some(t) => vec![PluginAction::OpenPublish {
                topic: t.topic,
                payload: t.payload,
                qos: t.qos,
                retain: t.retain,
            }],
            None => vec![PluginAction::Status(format!("no template named {id}"))],
        }
    }
}
