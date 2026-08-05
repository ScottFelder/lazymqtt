//! App-side logic for the connection form's subscription sub-list
//! (`Screen::SubscriptionList` / `Screen::SubscriptionForm`). The rows live on
//! the in-progress `FormBuffer.subs`; they're only persisted when the connection
//! form itself is saved.

use crate::app::{App, Screen, SubForm};
use crate::config::Subscription;

impl App {
    /// Open the subscription sub-list for the connection currently being edited.
    pub fn open_subscription_list(&mut self) {
        self.sub_list_selected = self
            .sub_list_selected
            .min(self.form.subs.len().saturating_sub(1));
        self.screen = Screen::SubscriptionList;
    }

    /// Open the form to add a new subscription.
    pub fn begin_subscription_add(&mut self) {
        self.sub_form = SubForm::default();
        self.screen = Screen::SubscriptionForm;
    }

    /// Open the form to edit the selected subscription.
    pub fn begin_subscription_edit(&mut self) {
        if let Some(sub) = self.form.subs.get(self.sub_list_selected) {
            self.sub_form = SubForm::from_subscription(self.sub_list_selected, sub);
            self.screen = Screen::SubscriptionForm;
        }
    }

    /// Delete the selected subscription, keeping the cursor in range.
    pub fn delete_selected_subscription(&mut self) {
        if self.sub_list_selected < self.form.subs.len() {
            self.form.subs.remove(self.sub_list_selected);
            self.sub_list_selected = self
                .sub_list_selected
                .min(self.form.subs.len().saturating_sub(1));
        }
    }

    /// Commit the subscription form into `form.subs` (add or replace) and return
    /// to the list. An empty topic is treated as cancel.
    pub fn commit_subscription_form(&mut self) {
        let topic = self.sub_form.topic.trim().to_string();
        if topic.is_empty() {
            self.screen = Screen::SubscriptionList;
            return;
        }
        let sub = Subscription {
            topic,
            qos: self.sub_form.qos.min(2),
        };
        match self.sub_form.editing_index {
            Some(i) if i < self.form.subs.len() => self.form.subs[i] = sub,
            _ => {
                self.form.subs.push(sub);
                self.sub_list_selected = self.form.subs.len() - 1;
            }
        }
        self.screen = Screen::SubscriptionList;
    }
}
