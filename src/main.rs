mod sim;
use sim::GPUSim;

use eframe::egui;

fn main() {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "GPU Simulation",
        native_options,
        Box::new(|cc| Box::new(GPUSimApp::new(cc))),
    )
    .unwrap();
}

pub struct GPUSimApp {
    sim: GPUSim,
}

impl GPUSimApp {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Self {
        let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();
        GPUSimApp {
            sim: GPUSim::new(wgpu_render_state, 800, 800, 20.),
        }
    }
}

impl eframe::App for GPUSimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    	egui::SidePanel::left("Settings").show(ctx, |ui| {
			ui.label("The triangle is being painted using ");
			// ui.hyperlink_to("WGPU", "https://wgpu.rs");
			ui.label(" (Portable Rust graphics API awesomeness)");
			ui.label("It's not a very impressive demo, but it shows you can embed 3D inside of egui.");
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.sim.params.dt, 0f32..=0.05));
                ui.label("dt");
            })
    	});
        egui::CentralPanel::default().show(ctx, |ui| {
			egui::Frame::canvas(ui.style()).show(ui, |ui| {
				let rect = ui.available_rect_before_wrap();
				ui.painter().add(eframe::egui_wgpu::Callback::new_paint_callback(
					rect,
					self.sim,
				));
			});
        });
        ctx.request_repaint();
    }
}