#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod twilight;
mod sim;
mod app;
use app::GPUSimApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`)
	let native_options = eframe::NativeOptions {
		renderer: eframe::Renderer::Wgpu,
		viewport: eframe::egui::ViewportBuilder::default()
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
						"<p> WebGPU is not available in your browser.</br>Please try a different browser or device. </p>",
					);
					log::error!("Failed to start eframe: {e:?}");
					panic!("Failed to start eframe: {e:?}");
				}
			}
		}
	});
}
