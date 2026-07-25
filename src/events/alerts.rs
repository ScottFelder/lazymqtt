use crate::app::{AlertForm, App, Screen};
use crossterm::event::{KeyCode, KeyEvent};

pub(crate) fn alert_rules_keys(app: &mut App, key: KeyEvent) {
    let len = app.alert_rules.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.screen = if app.handle.is_some() {
                Screen::Broker
            } else {
                Screen::Connections
            };
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                app.alerts_selected = (app.alerts_selected + 1).min(len - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.alerts_selected = app.alerts_selected.saturating_sub(1);
        }
        KeyCode::Char('a') => {
            app.alert_form = AlertForm::default();
            app.screen = Screen::AlertRuleForm;
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(rule) = app.alert_rules.get(app.alerts_selected) {
                app.alert_form = AlertForm::from_rule(app.alerts_selected, rule);
                app.screen = Screen::AlertRuleForm;
            }
        }
        KeyCode::Char('d') => {
            if app.alerts_selected < len {
                app.alert_rules.remove(app.alerts_selected);
                if app.alerts_selected > 0 && app.alerts_selected >= app.alert_rules.len() {
                    app.alerts_selected -= 1;
                }
                app.persist_alert_rules();
            }
        }
        _ => {}
    }
}

pub(crate) fn alert_form_keys(app: &mut App, key: KeyEvent) {
    let f = &mut app.alert_form;
    match key.code {
        KeyCode::Esc => app.screen = Screen::AlertRules,
        KeyCode::Tab | KeyCode::Down => f.focus = (f.focus + 1) % AlertForm::FIELD_COUNT,
        KeyCode::BackTab | KeyCode::Up => {
            f.focus = (f.focus + AlertForm::FIELD_COUNT - 1) % AlertForm::FIELD_COUNT
        }
        // `when` and `severity` are choice fields cycled with space.
        KeyCode::Char(' ') if f.focus == 1 => f.when = (f.when + 1) % AlertForm::WHEN_LABELS.len(),
        KeyCode::Char(' ') if f.focus == 5 => {
            f.severity = (f.severity + 1) % AlertForm::SEVERITY_LABELS.len()
        }
        KeyCode::Char(c) => match f.focus {
            0 => f.topic.push(c),
            2 if c.is_ascii_digit() || c == '.' || c == '-' => f.value.push(c),
            3 if c.is_ascii_digit() => f.seconds.push(c),
            4 => f.field.push(c),
            _ => {}
        },
        KeyCode::Backspace => match f.focus {
            0 => {
                f.topic.pop();
            }
            2 => {
                f.value.pop();
            }
            3 => {
                f.seconds.pop();
            }
            4 => {
                f.field.pop();
            }
            _ => {}
        },
        KeyCode::Enter => match app.alert_form.to_rule() {
            Ok(rule) => {
                match app.alert_form.editing_index {
                    Some(i) if i < app.alert_rules.len() => app.alert_rules[i] = rule,
                    _ => app.alert_rules.push(rule),
                }
                app.persist_alert_rules();
                app.screen = Screen::AlertRules;
            }
            Err(e) => app.error = Some(e),
        },
        _ => {}
    }
}
