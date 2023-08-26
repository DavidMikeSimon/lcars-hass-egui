use std::{collections::HashMap, env, sync::Arc, time::Duration};

use egui::{
    Button, Color32, Context, FontData, FontDefinitions, FontFamily, Key, Rect, Rounding, Sense,
    Stroke, Ui, Vec2, Visuals,
};
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

fn configure_fonts(ctx: &Context) {
    let mut font_definitions = FontDefinitions::empty();
    font_definitions.font_data.insert(
        "LCARSGTJ3".to_owned(),
        FontData::from_static(include_bytes!("../assets/LCARSGTJ3.ttf")),
    );
    font_definitions
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "LCARSGTJ3".to_owned());
    ctx.set_fonts(font_definitions);
}

fn configure_text_styles(ctx: &Context) {
    use egui::{FontId, TextStyle};
    use FontFamily::Proportional;

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(36.0, Proportional)),
        (TextStyle::Body, FontId::new(28.0, Proportional)),
        (TextStyle::Button, FontId::new(28.0, Proportional)),
        (TextStyle::Small, FontId::new(14.0, Proportional)),
    ]
    .into();
    ctx.set_style(style);
}

fn configure_visuals(ctx: &Context) {
    let mut visuals = Visuals::default();
    visuals.panel_fill = Color32::BLACK;
    ctx.set_visuals(visuals);
}

impl LcarsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_text_styles(&cc.egui_ctx);
        configure_visuals(&cc.egui_ctx);

        let runtime = runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // TODO: Use random unique id, or id from env var
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

struct LcarsPanel {
    bar_color: Color32,
    rounding: f32,
    sidebar_width: f32,
    header_height: f32,
    footer_height: f32,
}

impl Default for LcarsPanel {
    fn default() -> Self {
        Self {
            bar_color: Color32::DARK_BLUE,
            rounding: 30.0, // FIXME Doesn't seem to be applying correctly
            sidebar_width: 90.0,
            header_height: 50.0,
            footer_height: 10.0,
        }
    }
}

impl LcarsPanel {
    fn show(&self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
        let default_item_spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let height = 300.0;
        let width = 500.0;

        ui.horizontal(|ui| {
            {
                let (response, painter) =
                    ui.allocate_painter(Vec2::new(self.sidebar_width, height), Sense::click());
                painter.rect_filled(
                    response.rect,
                    Rounding {
                        nw: self.rounding,
                        sw: self.rounding,
                        ..Rounding::default()
                    },
                    self.bar_color,
                );
            }

            ui.vertical(|ui| {
                {
                    let (response, painter) =
                        ui.allocate_painter(Vec2::new(width, self.header_height), Sense::click());
                    painter.rect_filled(response.rect, Rounding::none(), self.bar_color);
                }

                ui.add_space(default_item_spacing.y);

                ui.horizontal(|ui| {
                    ui.add_space(default_item_spacing.x);

                    ui.spacing_mut().item_spacing = default_item_spacing;

                    ui.vertical(|ui| {
                        add_contents(ui);
                    });

                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                });

                {
                    let (response, painter) = ui
                        .allocate_painter(Vec2::new(width, ui.available_height()), Sense::click());
                    painter.rect_filled(
                        Rect::from_center_size(
                            response.rect.center_bottom(),
                            Vec2::new(width, self.footer_height),
                        ),
                        Rounding::none(),
                        self.bar_color,
                    );
                }
            });
        });
    }
}

impl eframe::App for LcarsApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
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

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            frame.close();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            LcarsPanel::default().show(ui, |ui| {
                let sleep_sound_state = self
                    .state
                    .device_states
                    .get("/switch/terminal1_sleepy_sounds_playing/state")
                    .map(|payload| match &**payload {
                        "on" => true,
                        _ => false,
                    });
                let btn = Button::new("SLP SND");
                let btn = match sleep_sound_state {
                    Some(true) => btn.fill(Color32::GREEN),
                    Some(false) => btn.fill(Color32::DARK_BLUE),
                    _ => btn,
                };
                let btn_response = ui.add_enabled(sleep_sound_state.is_some(), btn);
                if btn_response.clicked() {
                    let client = self.client.as_ref().clone();
                    self.runtime.spawn(async move {
                        client.publish(
                            "homeassistant_cmd/switch/terminal1_sleepy_sounds_playing",
                            QoS::AtLeastOnce,
                            false,
                            match sleep_sound_state {
                                Some(false) => "on",
                                _ => "off",
                            },
                        ).await.unwrap();
                    });
                }
            });
        });
    }
}
