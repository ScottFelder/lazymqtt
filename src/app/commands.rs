use crate::app::{App, Command, MenuAction, MenuItem, Screen, BROKER_COMMANDS};

impl App {
    /// Build the command-menu rows: the core `BROKER_COMMANDS` followed by each
    /// enabled plugin's commands (labels computed fresh, so they reflect current
    /// state). Shared by `open_command_menu` and the in-place adjust refresh.
    fn build_menu_items(&self) -> Vec<MenuItem> {
        let mut items: Vec<MenuItem> = BROKER_COMMANDS
            .iter()
            .map(|(cmd, key, desc)| MenuItem {
                key: key.to_string(),
                label: desc.to_string(),
                note: String::new(),
                action: MenuAction::Core(*cmd),
                adjustable: false,
            })
            .collect();
        let metadata = self.plugins.metadata();
        for (plugin, cmd) in self.plugins.commands() {
            let note = metadata
                .get(plugin)
                .map(|m| m.name.to_string())
                .unwrap_or_default();
            items.push(MenuItem {
                key: cmd.glyph.to_string(),
                label: cmd.label,
                note,
                action: MenuAction::Plugin { plugin, id: cmd.id },
                adjustable: cmd.adjustable,
            });
        }
        items
    }

    /// Build the command menu (core commands + enabled plugins' commands) and
    /// open it.
    pub fn open_command_menu(&mut self) {
        self.menu_items = self.build_menu_items();
        self.menu_selected = 0;
        self.screen = Screen::CommandMenu;
    }

    /// Invoke a plugin command from the menu and apply its actions. `OpenPane`
    /// is resolved here — we know the emitting plugin's index at this call site.
    pub fn invoke_plugin_command(&mut self, plugin: usize, id: &str) {
        let actions = self.plugins.invoke(plugin, id);
        if actions
            .iter()
            .any(|a| matches!(a, crate::plugin::PluginAction::OpenPane))
        {
            self.pane_plugin = plugin;
            self.screen = Screen::PluginPane;
        }
        self.apply_plugin_actions(actions);
    }

    /// Cycle the selected menu item's option in a direction (`forward` = right/
    /// `l`, else left/`h`), applying the plugin's actions and refreshing the
    /// menu labels in place. No-op for non-adjustable rows.
    pub fn adjust_selected_menu_item(&mut self, forward: bool) {
        let Some(item) = self.menu_items.get(self.menu_selected) else {
            return;
        };
        if !item.adjustable {
            return;
        }
        // Copy the target out so the immutable borrow of `self` ends before the
        // mutable plugin calls below.
        let MenuAction::Plugin { plugin, id } = &item.action else {
            return;
        };
        let (plugin, id) = (*plugin, id.clone());
        let actions = self.plugins.adjust(plugin, &id, forward);
        self.apply_plugin_actions(actions);
        // Rebuild so the row's label reflects the new option, keeping the cursor.
        self.menu_items = self.build_menu_items();
        self.menu_selected = self
            .menu_selected
            .min(self.menu_items.len().saturating_sub(1));
    }

    /// Run a broker command — the shared entry point for both its shortcut key
    /// and the command menu.
    pub fn run_command(&mut self, cmd: Command) {
        match cmd {
            Command::Subscribe => {
                self.sub_input.clear();
                self.screen = Screen::Subscribe;
            }
            Command::Unsubscribe => self.unsubscribe_selected(),
            Command::Publish => self.open_publish(),
            Command::ClearRetained => {
                if let Some(topic) = self.selected_topic() {
                    self.clear_topic = topic;
                    self.screen = Screen::ClearRetained;
                }
            }
            Command::ClearTopic => {
                if let Some(topic) = self.selected_topic() {
                    self.clear_topic_subtree(&topic);
                }
            }
            Command::ClearTree => {
                self.tree.clear();
                self.history.clear();
                self.expanded.clear();
                self.tree_selected = 0;
                self.reset_message_view();
            }
            Command::AlertRules => self.open_alerts_editor(),
            Command::Schemas => self.open_schemas(),
            Command::Recordings => self.open_recordings(),
            Command::Theme => self.open_theme(),
            Command::Plugins => {
                self.plugins_selected = 0;
                self.screen = Screen::Plugins;
            }
            Command::Help => self.screen = Screen::Help,
            Command::Disconnect => {
                self.disconnect();
                self.screen = Screen::Connections;
            }
            Command::Quit => self.should_quit = true,
        }
    }
}
