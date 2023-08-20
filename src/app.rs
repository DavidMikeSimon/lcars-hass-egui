use std::{time::Duration, env, sync::Arc};

use rumqttc::{MqttOptions, AsyncClient, QoS, EventLoop};
use tokio::{task, runtime, sync::mpsc};

pub struct LcarsApp {
    runtime: runtime::Runtime,
    
    message_receiver: mpsc::Receiver<String>,
    client: Arc<AsyncClient>,

    // Example stuff:
    label: String,
    value: f32,
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

        let client = Arc::new(client);

        let (sender, receiver) = mpsc::channel::<String>(1000);

        {
            let client = client.as_ref().clone();
            runtime.spawn(async move {
                client.subscribe("homeassistant_statestream/#", QoS::AtMostOnce).await.unwrap();
            });
        }

        {
            let sender = sender.clone();
            runtime.spawn(async move {
                loop {
                    match event_loop.poll().await {
                        Err(e) => {
                            println!("CONNECTION ERROR {:?}", e);
                            break;
                        },
                        Ok(m) => sender.send(format!("{:?}", m)).await.unwrap(),
                    }
                }
            });
        }

        Self {
            runtime,

            message_receiver: receiver,
            client: client,

            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
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
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { label, value, .. } = self;

        loop {
            match self.message_receiver.try_recv() {
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(_) => {
                    println!("WEIRD ERROR");
                    break
                },
                Ok(s) => {
                    println!("GOT {}", s);
                }
            }
        }

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(label);
            });

            ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                *value += 1.0;
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            ui.heading("eframe template");
            ui.hyperlink("https://github.com/emilk/eframe_template");
            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));
            egui::warn_if_debug_build(ui);
        });

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally choose either panels OR windows.");
            });
        }
    }
}
