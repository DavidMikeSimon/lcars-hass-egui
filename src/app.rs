use std::{time::Duration, env, sync::Arc, sync::RwLock, collections::HashMap};

use rumqttc::{MqttOptions, AsyncClient, QoS, Event, Packet};
use tokio::{runtime};

#[derive(Default)]
pub struct AppState {
    device_states: HashMap<String, String>,
}

pub struct LcarsApp {
    runtime: runtime::Runtime,
    state: Arc<RwLock<AppState>>,
    client: Arc<AsyncClient>,
}

impl Default for LcarsApp {
    fn default() -> Self {
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

        let state = Arc::new(RwLock::new(AppState::default()));

        let client = Arc::new(client);

        {
            let client = client.as_ref().clone();
            runtime.spawn(async move {
                client.subscribe("homeassistant_statestream/#", QoS::AtMostOnce).await.unwrap();
            });
        }

        {
            let state = state.clone();
            runtime.spawn(async move {
                loop {
                    match event_loop.poll().await {
                        Err(e) => {
                            println!("CONNECTION ERROR {:?}", e);
                            break;
                        },
                        Ok(Event::Incoming(Packet::Publish(p))) => {
                            println!("{:?}", &p);
                            if p.topic.ends_with("/state") {
                                let mut state = state.write().unwrap();
                                // FIXME: Too many copies happening here
                                state.device_states.insert(p.topic, String::from_utf8(p.payload.to_vec()).unwrap());
                            }
                        },
                        Ok(Event::Incoming(_)) => (),
                        Ok(Event::Outgoing(_)) => (),
                    }
                }
            });
        }

        Self {
            runtime,
            state,
            client: client,
        }
    }
}

impl LcarsApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        Default::default()
    }
}

impl eframe::App for LcarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let state = self.state.read().unwrap();
            ui.heading("eframe template");

            // FIXME Sorting every frame
            let mut keys: Vec<&String> = state.device_states.keys().collect();
            keys.sort();
            for key in keys {
                ui.label(format!("{}: {}", key, state.device_states.get(key).unwrap()));
            }
        });
    }
}
