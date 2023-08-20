use std::{time::Duration, env, sync::Arc, collections::HashMap};

use rumqttc::{MqttOptions, AsyncClient, QoS, Event, Packet};
use tokio::{runtime, sync::mpsc};

#[derive(Default)]
pub struct AppState {
    device_states: HashMap<String, String>,
}

pub struct LcarsApp {
    runtime: runtime::Runtime,
    state: AppState,
    client: Arc<AsyncClient>,
    event_receiver: mpsc::Receiver<Event>,
}

impl LcarsApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let runtime = runtime::Builder::new_multi_thread().enable_all().build().unwrap();

        // TODO: Use unique id
        let mut mqtt_options = MqttOptions::new("lcars", "mosquitto.sinclair.pipsimon.com", 1883);
        mqtt_options.set_credentials(
            "lcars",
            env::var("MQTT_PASS").unwrap()
        );
        mqtt_options.set_keep_alive(Duration::from_secs(5));

        let (client, mut event_loop) = AsyncClient::new(
            mqtt_options,
            10
        );

        let client = Arc::new(client);
        let (sender, receiver) = mpsc::channel::<Event>(1000);

        {
            let client = client.as_ref().clone();
            runtime.spawn(async move {
                client.subscribe("homeassistant_statestream/#", QoS::AtMostOnce).await.unwrap();
            });
        }

        {
            let context = cc.egui_ctx.clone();
            runtime.spawn(async move {
                loop {
                    match event_loop.poll().await {
                        Err(e) => {
                            // TODO Set some kind of flag? And reset it on next Ok?
                            println!("CONNECTION ERROR {:?}", e);
                            break;
                        },
                        Ok(event) => {
                            sender.send(event).await.unwrap();
                            context.request_repaint();
                            // TODO: Only request repaint after a batch of events
                        },
                    }
                }
            });
        }

        Self {
            runtime,
            state: AppState::default(),
            client: client,
            event_receiver: receiver,
        }
    }
}

impl eframe::App for LcarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.event_receiver.try_recv() {
            if let Event::Incoming(Packet::Publish(p)) = event {
                println!("{:?}", &p);
                if p.topic.ends_with("/state") {
                    // FIXME: Too many copies happening here
                    self.state.device_states.insert(p.topic, String::from_utf8(p.payload.to_vec()).unwrap());
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("eframe template");

            // FIXME Sorting every frame
            let mut keys: Vec<&String> = self.state.device_states.keys().collect();
            keys.sort();
            for key in keys {
                ui.label(format!("{}: {}", key, self.state.device_states.get(key).unwrap()));
            }
        });
    }
}
