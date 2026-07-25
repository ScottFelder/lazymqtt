mod app;
mod clipboard;
mod config;
mod events;
mod mqtt;
mod plugin;
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
use plugin::PluginEvent;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

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
    // Plugins get a Tick roughly once a second (for silence detection, rolling
    // stats, etc.) and a faster FrameTick (~10 Hz) for time-sensitive work like
    // replay pacing — both decoupled from the input-poll cadence.
    let mut last_tick = Instant::now();
    let mut last_frame = Instant::now();

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
                MqttEvent::Connected => {
                    app.status = Status::Connected;
                    let id = app.active_conn().map(|c| c.id.clone()).unwrap_or_default();
                    app.dispatch_plugin(PluginEvent::Connected { connection: id });
                }
                MqttEvent::Disconnected(e) => {
                    app.status = Status::Disconnected(e.clone());
                    app.dispatch_plugin(PluginEvent::Disconnected(e));
                }
                MqttEvent::Error(e) => app.error = Some(e),
                // push_message dispatches MessageReceived itself.
                MqttEvent::Message(m) => app.push_message(m),
            }
        }

        if last_tick.elapsed() >= Duration::from_secs(1) {
            app.dispatch_plugin(PluginEvent::Tick);
            last_tick = Instant::now();
        }
        if last_frame.elapsed() >= Duration::from_millis(100) {
            app.dispatch_plugin(PluginEvent::FrameTick);
            last_frame = Instant::now();
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
            app.dispatch_plugin(PluginEvent::Shutdown);
            app.send(mqtt::MqttCommand::Disconnect);
            break;
        }
    }
    Ok(())
}
