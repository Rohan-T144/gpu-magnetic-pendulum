use std::num::NonZeroU64;

use egui::*;
use egui_winit::{winit as winit, State};
use egui_wgpu::{wgpu::{self as wgpu, include_wgsl, util::DeviceExt}, Renderer, ScreenDescriptor};
use wgpu::{CommandEncoder, Device, Queue, StoreOp, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;
use glam::{vec2, Vec2};
use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Particle {
	u: Vec2,
	du: Vec2,
}

const W: u32 = 1360/2;
const H: u32 = 768/2;
// const N: u32 = W*H;

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Params {
	r: f32, // radius of the magnets from centre
	d: f32,
	mu: f32, // coefficient of friction
	c: f32,
	w: u32,
	h: u32
}

const PARAMS: Params = Params{
	r: 2.0,
	d: 0.2,
	mu: 0.12,
	c: 0.2,
	w: W as u32,
	h: H as u32,
};
const SCALE: f32 = 16.;

pub struct EguiRenderer {
	state: State,
	renderer: Renderer,
	compute_pipeline: wgpu::ComputePipeline,
	bind_group: wgpu::BindGroup,
	render_pipeline: wgpu::RenderPipeline,
	render_bg: wgpu::BindGroup,
}

impl EguiRenderer {
	// pub fn context(&self) -> &Context {
	//     self.state.egui_ctx()
	// }
	pub fn new(
		device: &Device,
		output_color_format: TextureFormat,
		output_depth_format: Option<TextureFormat>,
		msaa_samples: u32,
		window: &Window,
	) -> EguiRenderer {
		let particles: Vec<_>  = (0..W*H)
			.map(|i| Particle{
				u: (vec2((i%W) as f32/W as f32, (i/W) as f32/H as f32)-Vec2::splat(0.5))*SCALE,
				du: Vec2::ZERO
			})
			.collect();

		let param_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
			label: Some("particles"),
			contents: bytemuck::cast_slice(&[PARAMS]),
			usage: wgpu::BufferUsages::UNIFORM
		});


		let particle_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
			label: Some("particles"),
			contents: bytemuck::cast_slice(&particles),
			usage: wgpu::BufferUsages::STORAGE
		});

		let bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("Avaialable Buffers"),
			entries: &[
				// Simulation Parameters
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None
					},
					count: None,
				},
				// Old Particle data
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::COMPUTE,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Storage { read_only: false },
						has_dynamic_offset: false,
						min_binding_size: None
					},
					count: None,
				},
				// The texture
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::COMPUTE,
					ty: wgpu::BindingType::StorageTexture {
						access: wgpu::StorageTextureAccess::WriteOnly,
						format: wgpu::TextureFormat::Rgba8Unorm,
						view_dimension: wgpu::TextureViewDimension::D2
					},
					count: None,
				},
			],
		});

		let shader_module = device.create_shader_module(include_wgsl!("shader.wgsl"));

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("update layout"),
			bind_group_layouts: &[&bg_layout],
			push_constant_ranges: &[],
		});

		let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
			label: Some("Compute pipeline"),
			layout: Some(&pipeline_layout),
			module:  &shader_module,
			entry_point: "comp_main",
		});

		let egui_context = Context::default();

		let egui_state = egui_winit::State::new(
			egui_context,
			egui::viewport::ViewportId::ROOT,
			&window,
			Some(window.scale_factor() as f32),
			None,
		);
		let egui_renderer = Renderer::new(
			device,
			output_color_format,
			output_depth_format,
			msaa_samples,
		);
		let tex = device.create_texture(
			&wgpu::TextureDescriptor {
				label: Some("magpen texture"),
				size: wgpu::Extent3d{width: W, height:H ,..Default::default()},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba8Unorm,
				usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
				view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
			}
		);
		let texview = tex.create_view(&wgpu::TextureViewDescriptor{
			label: Some("magpen texture id"),
			..Default::default()
		});

		let render_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("Render Layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true }
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
		});
		let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("render pipeline layout"),
			bind_group_layouts: &[&render_bg_layout],
			push_constant_ranges: &[],
		});
		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
			label: Some("Render pipeline"),
			layout: Some(&render_pipeline_layout),
			vertex: wgpu::VertexState{
				module: &shader_module,
				entry_point: "vs_main",
				buffers: &[]
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader_module,
				entry_point: "fs_main",
				targets: &[Some(wgpu::ColorTargetState {
					format: output_color_format,
					blend: Some(wgpu::BlendState::REPLACE),
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleStrip,
				..Default::default()
			},
			multisample: wgpu::MultisampleState::default(),
			depth_stencil: None,
			multiview: None

			// strip_index_format: (), front_face: (), cull_mode: (), unclipped_depth: (), polygon_mode: (), conservative: () }
		});
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor{
			label: Some("sampler"),
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &render_bg_layout,
			label: Some("Resources described by the render_bg_layout"),
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&texview)
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&sampler)
				},
			],
		});

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &bg_layout,
			label: Some("Resources described by the bind_group_layout"),
			entries: &[
				// params
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: &param_buf,
						offset: 0,
						// size: None,
						size: NonZeroU64::new(std::mem::size_of::<Params>() as u64)
					}),
				},
				// particles
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: &particle_buf,
						offset: 0,
						size: NonZeroU64::new(((W*H) as usize * std::mem::size_of::<Particle>()) as u64),
						// size: NonZeroU64::new(particles_size)
					}),
				},
				// Current Particle data
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&texview)
				},
			],
		});

		EguiRenderer {
			state: egui_state,
			renderer: egui_renderer,
			compute_pipeline,
			bind_group,
			render_pipeline,
			render_bg
		}
	}

	pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) -> egui_winit::EventResponse {
		self.state.on_window_event(window, &event)
	}
	// pub fn ppp(&mut self, v: f32) {
	//     self.state.egui_ctx().set_pixels_per_point(v);
	// }

	pub fn draw(
		&mut self,
		device: &Device,
		queue: &Queue,
		encoder: &mut CommandEncoder,
		window: &Window,
		window_surface_view: &TextureView,
		screen_descriptor: ScreenDescriptor,
	) {
		self.state
			.egui_ctx()
			.set_pixels_per_point(screen_descriptor.pixels_per_point);
	{
		let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor{
			label: Some("Compute pass"),
			timestamp_writes: None
		});
		cpass.set_pipeline(&self.compute_pipeline);
		cpass.set_bind_group(0, &self.bind_group, &[]);

		cpass.dispatch_workgroups(W, H, 1);
	}
	{
		let mut mrpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
			label: Some("magpen render pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &window_surface_view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: StoreOp::Store,
				},
			})],
			depth_stencil_attachment: None,
			timestamp_writes: None,
			occlusion_query_set: None,
		});

		mrpass.set_bind_group(0, &self.render_bg, &[]);
		mrpass.set_pipeline(&self.render_pipeline);

		mrpass.draw(0..4, 0..1);
	}

		let raw_input = self.state.take_egui_input(&window);
		let full_output = self.state.egui_ctx().run(raw_input, |ctx| {
			egui::SidePanel::left("settings_panel").show(&ctx, |ui| {
				ui.label("Label!");
				ui.separator();
				if ui.button("Button!").clicked() {
					println!("test")
				}
			});
		});

		self.state.handle_platform_output(&window, full_output.platform_output);

		let tris = self.state.egui_ctx()
			.tessellate(full_output.shapes, self.state.egui_ctx().pixels_per_point());
		for (id, image_delta) in &full_output.textures_delta.set {
			self.renderer.update_texture(&device, &queue, *id, &image_delta);
		}
		self.renderer.update_buffers(&device, &queue, encoder, &tris, &screen_descriptor);
	{
		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &window_surface_view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: StoreOp::Store,
				},
			})],
			depth_stencil_attachment: None,
			timestamp_writes: None,
			label: Some("egui main render pass"),
			occlusion_query_set: None,
		});
		self.renderer.render(&mut rpass, &tris, &screen_descriptor);
	}
		for x in &full_output.textures_delta.free {
			self.renderer.free_texture(x)
		}
	}
}
