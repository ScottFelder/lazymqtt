use crate::app::{App, Command, MenuAction, MenuItem, Screen, BROKER_COMMANDS};

impl App {
    /// Build the command-menu rows for the current level. The top level is the
    /// core `BROKER_COMMANDS` plus one submenu entry per plugin that has
    /// commands; a plugin submenu (`menu_plugin` set) is that plugin's own
    /// commands. Rebuilt on open and after each in-place adjust so labels stay
    /// current.
    fn build_menu_items(&self) -> Vec<MenuItem> {
        if let Some(plugin) = self.menu_plugin {
            return self
                .plugins
                .commands()
                .into_iter()
                .filter(|(i, _)| *i == plugin)
                .map(|(plugin, cmd)| MenuItem {
                    key: cmd.glyph.to_string(),
                    label: cmd.label,
                    note: String::new(),
                    action: MenuAction::Plugin { plugin, id: cmd.id },
                    adjustable: cmd.adjustable,
                })
                .collect();
        }

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
        // One submenu opener per enabled plugin that contributes commands.
        for (plugin, meta) in self.plugins.command_plugins() {
            items.push(MenuItem {
                key: "▸".to_string(),
                label: meta.name.to_string(),
                note: meta.description.to_string(),
                action: MenuAction::Submenu(plugin),
                adjustable: false,
            });
        }
        items
    }

    /// Open the command menu at its top level.
    pub fn open_command_menu(&mut self) {
        self.menu_plugin = None;
        self.menu_items = self.build_menu_items();
        self.menu_selected = 0;
        self.screen = Screen::CommandMenu;
    }

    /// Descend into a plugin's submenu (its own commands).
    pub fn open_plugin_submenu(&mut self, plugin: usize) {
        self.menu_plugin = Some(plugin);
        self.menu_items = self.build_menu_items();
        self.menu_selected = 0;
    }

    /// Return from a plugin submenu to the top-level menu.
    pub fn close_plugin_submenu(&mut self) {
        self.menu_plugin = None;
        self.menu_items = self.build_menu_items();
        self.menu_selected = 0;
    }

    /// The current submenu's plugin name, if in one (for the popup title).
    pub fn menu_plugin_name(&self) -> Option<String> {
        self.menu_plugin
            .and_then(|i| self.plugins.metadata().get(i).map(|m| m.name.to_string()))
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

    /// Activate the currently selected menu row — the shared behavior for both
    /// `Enter` and an item's accelerator key. A submenu opener descends; an
    /// adjustable row cycles its option forward in place; a concrete command
    /// closes the menu and runs.
    pub fn activate_selected_menu_item(&mut self) {
        let Some(item) = self.menu_items.get(self.menu_selected) else {
            return;
        };
        let adjustable = item.adjustable;
        let action = item.action.clone();
        match action {
            MenuAction::Submenu(plugin) => self.open_plugin_submenu(plugin),
            _ if adjustable => self.adjust_selected_menu_item(true),
            MenuAction::Core(cmd) => {
                self.screen = Screen::Broker;
                self.run_command(cmd);
            }
            MenuAction::Plugin { plugin, id } => {
                self.screen = Screen::Broker;
                self.invoke_plugin_command(plugin, &id);
            }
        }
    }

    /// Select and activate the menu row whose accelerator key matches `c`, if
    /// any. Returns whether a row was matched.
    pub fn activate_menu_key(&mut self, c: char) -> bool {
        let mut buf = [0u8; 4];
        let typed: &str = c.encode_utf8(&mut buf);
        let Some(i) = self.menu_items.iter().position(|it| it.key == typed) else {
            return false;
        };
        self.menu_selected = i;
        self.activate_selected_menu_item();
        true
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
            Command::ExpandSubtree => self.expand_subtree(),
            Command::CollapseSubtree => self.collapse_subtree(),
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
