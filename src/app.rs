use std::{collections::HashMap, env, sync::Arc, time::Duration};

use egui::{FontData, FontDefinitions};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
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

fn configure_fonts(ctx: &egui::Context) {
    let mut font_definitions = FontDefinitions::empty();
    font_definitions.font_data.insert(
        "LCARSGTJ3".to_owned(),
        FontData::from_static(include_bytes!("../assets/LCARSGTJ3.ttf")),
    );
    font_definitions
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "LCARSGTJ3".to_owned());
    ctx.set_fonts(font_definitions);
}

fn configure_text_styles(ctx: &egui::Context) {
    use egui::FontFamily::{Monospace, Proportional};
    use egui::{FontId, TextStyle};

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(36.0, Proportional)),
        (TextStyle::Body, FontId::new(28.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
        (TextStyle::Button, FontId::new(12.0, Proportional)),
        (TextStyle::Small, FontId::new(8.0, Proportional)),
    ]
    .into();
    ctx.set_style(style);
}

impl LcarsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_text_styles(&cc.egui_ctx);

        let runtime = runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // TODO: Use unique id
        let mut mqtt_options = MqttOptions::new("lcars", "mosquitto.sinclair.pipsimon.com", 1883);
        mqtt_options.set_credentials("lcars", env::var("MQTT_PASS").unwrap());
        mqtt_options.set_keep_alive(Duration::from_secs(5));

        let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

        let client = Arc::new(client);
        let (sender, receiver) = mpsc::channel::<Event>(1000);

        {
            let client = client.as_ref().clone();
            runtime.spawn(async move {
                client
                    .subscribe("homeassistant_statestream/#", QoS::AtMostOnce)
                    .await
                    .unwrap();
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
                        }
                        Ok(event) => {
                            sender.send(event).await.unwrap();
                            context.request_repaint();
                            // TODO: Only request repaint after a batch of events
                        }
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
                if p.topic.ends_with("/state") {
                    // FIXME: Too many copies happening here
                    self.state.device_states.insert(
                        p.topic.replace("homeassistant_statestream", ""),
                        String::from_utf8(p.payload.to_vec()).unwrap(),
                    );
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // FIXME Sorting every frame
            let mut keys: Vec<&String> = self.state.device_states.keys().collect();
            keys.sort();
            for key in keys {
                ui.label(format!(
                    "{}: {}",
                    key,
                    self.state.device_states.get(key).unwrap()
                ));
            }
        });
    }
}
