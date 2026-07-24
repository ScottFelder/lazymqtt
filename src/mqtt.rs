use crate::config::Connection;
use anyhow::Result;
use chrono::{DateTime, Local};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, Outgoing, QoS, Transport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retained: bool,
    pub time: DateTime<Local>,
}

/// Events flowing from the MQTT task up to the UI.
#[derive(Debug, Clone)]
pub enum MqttEvent {
    Connected,
    Disconnected(String),
    Message(Message),
    Error(String),
}

/// Commands flowing from the UI down to the MQTT task.
#[derive(Debug, Clone)]
pub enum MqttCommand {
    Subscribe {
        topic: String,
        qos: u8,
    },
    Unsubscribe {
        topic: String,
    },
    Publish {
        topic: String,
        payload: String,
        qos: u8,
        retain: bool,
    },
    Disconnect,
}

pub fn qos_from(v: u8) -> QoS {
    match v {
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtMostOnce,
    }
}

/// Handle held by the UI to talk to a live connection.
pub struct MqttHandle {
    pub events: UnboundedReceiver<MqttEvent>,
    pub commands: UnboundedSender<MqttCommand>,
}

pub fn connect(conn: &Connection) -> Result<MqttHandle> {
    let (ev_tx, ev_rx) = tokio::sync::mpsc::unbounded_channel::<MqttEvent>();
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<MqttCommand>();

    let mut opts = MqttOptions::new(conn.client_id.clone(), conn.host.clone(), conn.port);
    opts.set_keep_alive(Duration::from_secs(30));
    // rumqttc defaults to a 10KB max packet size for both directions, which is
    // too small for many real-world payloads (e.g. JSON/binary sensor blobs)
    // and causes "payload size limit exceeded" errors that tear down and
    // reconnect the eventloop in a loop. Raise both limits generously.
    opts.set_max_packet_size(10 * 1024 * 1024, 10 * 1024 * 1024);
    if !conn.username.is_empty() {
        opts.set_credentials(conn.username.clone(), conn.password.clone());
    }
    if conn.tls {
        opts.set_transport(Transport::tls_with_default_config());
    }

    let subs = conn.subscriptions.clone();
    let (client, mut eventloop) = AsyncClient::new(opts, 64);

    // Set once the UI asks to disconnect. rumqttc's eventloop reconnects on its
    // own as long as it keeps being polled, so a user disconnect must both stop
    // the command pump AND break the event loop — otherwise the old connection
    // lives on, reconnecting forever, and fights the next connection for the
    // same client id ("connection closed by peer").
    let stop = Arc::new(AtomicBool::new(false));

    // Command pump: forward UI commands to the client.
    {
        let client = client.clone();
        let ev_tx = ev_tx.clone();
        let stop = stop.clone();
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let res = match cmd {
                    MqttCommand::Subscribe { topic, qos } => {
                        client.subscribe(topic, qos_from(qos)).await
                    }
                    MqttCommand::Unsubscribe { topic } => client.unsubscribe(topic).await,
                    MqttCommand::Publish {
                        topic,
                        payload,
                        qos,
                        retain,
                    } => {
                        client
                            .publish(topic, qos_from(qos), retain, payload.into_bytes())
                            .await
                    }
                    MqttCommand::Disconnect => {
                        stop.store(true, Ordering::SeqCst);
                        let _ = client.disconnect().await;
                        break;
                    }
                };
                if let Err(e) = res {
                    let _ = ev_tx.send(MqttEvent::Error(e.to_string()));
                }
            }
        });
    }

    // Event loop: drive the connection and surface incoming packets.
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                    let _ = ev_tx.send(MqttEvent::Connected);
                    for s in &subs {
                        let _ = client.subscribe(s.topic.clone(), qos_from(s.qos)).await;
                    }
                }
                Ok(Event::Incoming(Incoming::Publish(p))) => {
                    let msg = Message {
                        topic: p.topic,
                        payload: String::from_utf8_lossy(&p.payload).to_string(),
                        qos: match p.qos {
                            QoS::AtMostOnce => 0,
                            QoS::AtLeastOnce => 1,
                            QoS::ExactlyOnce => 2,
                        },
                        retained: p.retain,
                        time: Local::now(),
                    };
                    let _ = ev_tx.send(MqttEvent::Message(msg));
                }
                // Our own disconnect went out: the connection is closing for good.
                Ok(Event::Outgoing(Outgoing::Disconnect)) => break,
                Ok(_) => {}
                Err(e) => {
                    // Don't announce or retry a disconnect the user asked for.
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let _ = ev_tx.send(MqttEvent::Disconnected(e.to_string()));
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }
    });

    Ok(MqttHandle {
        events: ev_rx,
        commands: cmd_tx,
    })
}
