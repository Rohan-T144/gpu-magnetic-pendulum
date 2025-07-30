#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod resources;
mod sim;
use std::{error::Error, sync::Arc};
use sim::{GPUSim, Params};
use eframe::egui::{self, FontData, FontDefinitions, Sense, Slider, Vec2};
use rand::Rng;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`)
	let native_options = eframe::NativeOptions {
		renderer: eframe::Renderer::Wgpu,
		viewport: egui::ViewportBuilder::default()
			.with_inner_size([1200.0, 800.0])
			.with_min_inner_size([800.0, 600.0])
			.with_resizable(true),
		..Default::default()
	};
	eframe::run_native(
		"GPU Simulation",
		native_options,
		Box::new(|cc| Ok(Box::new(GPUSimApp::new(cc)))),
	)?;
	Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
	use eframe::wasm_bindgen::JsCast as _;
	// Redirect `log` message to `console.log` and friends:
	eframe::WebLogger::init(log::LevelFilter::Debug).ok();
	let web_options = eframe::WebOptions::default();
	wasm_bindgen_futures::spawn_local(async {
		let document = web_sys::window()
			.expect("No window")
			.document()
			.expect("No document");
		let canvas = document
			.get_element_by_id("the_canvas_id")
			.expect("Failed to find the_canvas_id")
			.dyn_into::<web_sys::HtmlCanvasElement>()
			.expect("the_canvas_id was not a HtmlCanvasElement");
		let start_result = eframe::WebRunner::new()
			.start(
				canvas,
				web_options,
				Box::new(|cc| Ok(Box::new(GPUSimApp::new(cc)))),
			)
			.await;
		// Remove the loading text and spinner:
		if let Some(loading_text) = document.get_element_by_id("loading_text") {
			match start_result {
				Ok(_) => {
					loading_text.remove();
				}
				Err(e) => {
					loading_text.set_inner_html(
						"<p> The app has crashed. See the developer console for details. </p>",
					);
					panic!("Failed to start eframe: {e:?}");
				}
			}
		}
	});
}

pub struct GPUSimApp {
	sim: GPUSim,
	is_paused: bool,
	width: u32,
	height: u32,
	_scale: f32,
}

impl GPUSimApp {
	pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Self {
		let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();
		let width = 1024;
		let height = 1024;
		let scale = 25.;
		let mut fonts = FontDefinitions::default();
		fonts.font_data.insert(
			"Inter".to_owned(),
			Arc::new(FontData::from_static(include_bytes!("../fonts/InterVariable.ttf"))),
		);
		fonts
			.families
			.get_mut(&egui::FontFamily::Proportional)
			.unwrap()
			.insert(0, "Inter".to_owned());
		cc.egui_ctx.set_fonts(fonts);
		cc.egui_ctx.options_mut(|o| o.screen_reader = true);
		GPUSimApp {
			sim: GPUSim::new(wgpu_render_state, width, height, scale),
			is_paused: true,
			width,
			height,
			_scale: scale,
		}
	}
}

impl eframe::App for GPUSimApp {
	fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
		egui::SidePanel::left("Settings").show(ctx, |ui| {
			ui.heading("GPU Magnetic Pendulum Simulation");
			ui.separator();
			
			// Play/Pause button
			ui.horizontal(|ui| {
				// ⏸ is not in the Inter font
				if ui.button(if self.is_paused { "▶ Play" } else { "■ Pause" }).clicked() {
					self.is_paused = !self.is_paused;
				}
				ui.label(if self.is_paused { "Simulation Paused" } else { "Simulation Running" });
			});
			ui.separator();
			
			ui.label("Simulation Parameters:");
			ui.add_space(10.0);
			
			// Number of magnets
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.n, 3..=10));
				ui.label("Number of magnets");
			});
			
			// Magnet radius from center
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.r, 1.0..=10.0).step_by(0.1));
				ui.label("Magnet radius from center");
			});
			
			// Distance parameter
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.d, 0.1..=2.0).step_by(0.01));
				ui.label("Distance parameter");
			});
			
			// Friction coefficient
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.mu, 0.0..=1.0).step_by(0.01));
				ui.label("Friction coefficient");
			});
			
			// Spring constant
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.c, 0.0..=1.0).step_by(0.01));
				ui.label("Spring constant");
			});
			
			// Time step
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.dt, 0.001..=0.05).step_by(0.001));
				ui.label("Time step (dt)");
			});
			
			ui.separator();
			ui.label("Initial Velocity Settings:");
			ui.add_space(5.0);
			
			// Velocity magnitude
			ui.horizontal(|ui| {
				ui.add(Slider::new(&mut self.sim.params.velocity_magnitude, 0.0..=10.0)
					.step_by(0.1)
					.text("magnitude"))
					.on_hover_text("Controls how fast particles start moving");
				ui.label("Initial speed");
			});
			
			// Velocity angle
			ui.horizontal(|ui| {
				let mut angle_degrees = self.sim.params.velocity_angle.to_degrees();
				if ui.add(Slider::new(&mut angle_degrees, 0.0..=360.0)
					.step_by(1.0)
					.text("angle"))
					.on_hover_text("Rotation offset for velocity directions")
					.changed() {
					self.sim.params.velocity_angle = angle_degrees.to_radians();
				}
				ui.label("Velocity angle (°)");
			});
			
			// Velocity pattern
			ui.horizontal(|ui| {
				ui.label("Velocity pattern:")
					.on_hover_text("How initial velocities are distributed across particles");
				egui::ComboBox::from_id_salt("velocity_pattern")
					.selected_text(match self.sim.params.velocity_pattern {
						0 => "Radial (→)",
						1 => "Tangential (↻)", 
						2 => "Uniform (↗)",
						_ => "Zero (○)",
					})
					.show_ui(ui, |ui| {
						ui.selectable_value(&mut self.sim.params.velocity_pattern, 0, "Radial (→) - outward from center");
						ui.selectable_value(&mut self.sim.params.velocity_pattern, 1, "Tangential (↻) - circular motion");
						ui.selectable_value(&mut self.sim.params.velocity_pattern, 2, "Uniform (↗) - same direction");
						ui.selectable_value(&mut self.sim.params.velocity_pattern, 3, "Zero (○) - start at rest");
					});
			});
			
			// Visual indicator for velocity pattern
			ui.horizontal(|ui| {
				ui.label("Preview:");
				let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 60.0), egui::Sense::hover());
				let painter = ui.painter();
				let center = rect.center();
				let radius = 25.0;
				
				// Draw background circle
				painter.circle_stroke(center, radius, egui::Stroke::new(1.0, egui::Color32::GRAY));
				
				// Draw velocity vectors based on pattern
				let num_samples = 8;
				for i in 0..num_samples {
					let angle = (i as f32) * std::f32::consts::TAU / (num_samples as f32);
					let pos = center + Vec2::angled(angle) * (radius * 0.7);
					
					let velocity_dir = match self.sim.params.velocity_pattern {
						0 => Vec2::angled(angle + self.sim.params.velocity_angle), // radial
						1 => Vec2::angled(angle + std::f32::consts::PI / 2.0 + self.sim.params.velocity_angle), // tangential
						2 => Vec2::angled(self.sim.params.velocity_angle), // uniform
						_ => Vec2::ZERO, // zero
					};
					
					if velocity_dir != Vec2::ZERO {
						painter.arrow(pos, velocity_dir * 8.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 150, 255)));
					}
					
					// Draw position dots
					painter.circle_filled(pos, 2.0, egui::Color32::WHITE);
				}
			});
			
			ui.separator();
			ui.add_space(10.0);
			
			// Reset and restart buttons
			ui.horizontal(|ui| {
				if ui.button("Reset Parameters").clicked() {
					self.sim.params = Params::default(self.width, self.height);
				}
				
				if ui.button("Restart Simulation").clicked() {
					// Reset particles to initial positions
					if let Some(wgpu_render_state) = frame.wgpu_render_state() {
						self.sim.restart(wgpu_render_state);
						self.is_paused = true;
					}
				}
			});
			
			// Randomize velocity button
			if ui.button("Randomize Velocity").clicked() {
				let mut rng = rand::rng();
				self.sim.params.velocity_magnitude = rng.random_range(0.5..8.0);
				self.sim.params.velocity_angle = rng.random_range(0.0..std::f32::consts::TAU);
				self.sim.params.velocity_pattern = rng.random_range(0..4);
			}
			
			ui.separator();
			ui.label("Presets:");
			ui.horizontal(|ui| {
				if ui.button("Chaotic").clicked() {
					self.sim.params.n = 3;
					self.sim.params.r = 2.5;
					self.sim.params.d = 0.2;
					self.sim.params.mu = 0.05;
					self.sim.params.c = 0.1;
					self.sim.params.dt = 0.008;
					self.sim.params.velocity_magnitude = 6.0;
					self.sim.params.velocity_angle = 0.0;
					self.sim.params.velocity_pattern = 0; // radial
				}
				
				if ui.button("Smooth").clicked() {
					self.sim.params.n = 5;
					self.sim.params.r = 4.0;
					self.sim.params.d = 0.6;
					self.sim.params.mu = 0.4;
					self.sim.params.c = 0.3;
					self.sim.params.dt = 0.004;
					self.sim.params.velocity_magnitude = 2.0;
					self.sim.params.velocity_angle = std::f32::consts::PI / 4.0;
					self.sim.params.velocity_pattern = 1; // tangential
				}
			});
			
			ui.horizontal(|ui| {
				if ui.button("Complex").clicked() {
					self.sim.params.n = 7;
					self.sim.params.r = 3.5;
					self.sim.params.d = 0.3;
					self.sim.params.mu = 0.15;
					self.sim.params.c = 0.25;
					self.sim.params.dt = 0.005;
					self.sim.params.velocity_magnitude = 5.0;
					self.sim.params.velocity_angle = std::f32::consts::PI;
					self.sim.params.velocity_pattern = 2; // uniform
				}
				
				if ui.button("Stable").clicked() {
					self.sim.params.n = 4;
					self.sim.params.r = 3.0;
					self.sim.params.d = 0.8;
					self.sim.params.mu = 0.6;
					self.sim.params.c = 0.4;
					self.sim.params.dt = 0.003;
					self.sim.params.velocity_magnitude = 1.0;
					self.sim.params.velocity_angle = 0.0;
					self.sim.params.velocity_pattern = 3; // zero
				}
			});
			
			ui.add_space(20.0);
			ui.label("About:");
			ui.label("This simulation shows the chaotic motion of a magnetic pendulum under the influence of multiple magnets.");
			ui.label("• Higher friction (μ) creates smoother patterns");
			ui.label("• Lower friction creates more chaotic behavior");
			ui.label("• Spring constant (c) affects restoring force");
			ui.label("• Distance parameter (d) controls singularity smoothing");
			ui.label("• Velocity patterns:");
			ui.label("  - Radial: velocities point outward from center");
			ui.label("  - Tangential: velocities perpendicular to position");
			ui.label("  - Uniform: all particles have same direction");
			ui.label("  - Zero: particles start at rest");
		});

		egui::CentralPanel::default()
			.frame(egui::Frame::NONE.inner_margin(15.0)) // Remove default frame styling
			.show(ctx, |ui| {
			egui::Frame::NONE.show(ui, |ui| {
				// Use all available space for the simulation
				let available_size = ui.available_size();
				
				// Make it square and use the smaller dimension to fit properly
				let min_dimension = available_size.x.min(available_size.y).max(200.0); // Minimum size of 200px
				let canvas_size = egui::vec2(min_dimension, min_dimension);
				let (resp, ptr) = ui.allocate_painter(available_size, Sense::focusable_noninteractive());
				let canv_rect = egui::Rect::from_center_size(resp.rect.center(), canvas_size);

				// Only update simulation if not paused
				if !self.is_paused {
					ptr.add(eframe::egui_wgpu::Callback::new_paint_callback(
						canv_rect, self.sim,
					));
				} else {
					// When paused, still render the current state but don't update
					let mut paused_sim = self.sim;
					paused_sim.params.dt = 0.0; // Set dt to 0 to prevent updates
					ptr.add(eframe::egui_wgpu::Callback::new_paint_callback(
						canv_rect, paused_sim,
					));
				}
			});
		});

		// Only request repaint if not paused, or always for UI updates
		ctx.request_repaint();
	}
}
