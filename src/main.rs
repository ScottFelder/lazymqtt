mod app;
mod clipboard;
mod config;
mod events;
mod mqtt;
mod tree;
mod ui;

use anyhow::Result;
use app::{App, Status};
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mqtt::MqttEvent;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // No mouse capture: leaving the mouse to the terminal keeps native
    // click-drag text selection (and copy) working in the message pane.
    // Bracketed paste lets a paste arrive as a single Event::Paste.
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("error: {e}");
    }
    Ok(())
}

async fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Drain any pending MQTT events without blocking the render loop.
        // Collect first so the handle borrow ends before we mutate `app`.
        let mut pending = Vec::new();
        if let Some(handle) = &mut app.handle {
            while let Ok(ev) = handle.events.try_recv() {
                pending.push(ev);
            }
        }
        for ev in pending {
            match ev {
                MqttEvent::Connected => app.status = Status::Connected,
                MqttEvent::Disconnected(e) => app.status = Status::Disconnected(e),
                MqttEvent::Error(e) => app.error = Some(e),
                MqttEvent::Message(m) => app.push_message(m),
            }
        }

        terminal.draw(|f| ui::draw(f, app))?;

        // Poll input with a short timeout so live messages keep flowing in.
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    events::handle_key(app, key);
                }
                Event::Paste(data) => events::handle_paste(app, data),
                _ => {}
            }
        }

        if app.should_quit {
            app.send(mqtt::MqttCommand::Disconnect);
            break;
        }
    }
    Ok(())
}
