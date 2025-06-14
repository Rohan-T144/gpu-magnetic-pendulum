use std::{f32::consts::PI, num::NonZeroU64};

use bytemuck::{Pod, Zeroable};
use eframe::egui::*;
use eframe::egui_wgpu::ScreenDescriptor;
use eframe::wgpu;
use glam::{vec2, Vec2};
use wgpu::{include_wgsl, util::DeviceExt, TextureFormat};

use crate::resources::TWILIGHT_MAP;

// wgpu requires the structures to be padded to 16 bytes (4 floats)
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct Particle {
    u: Vec2,
    du: Vec2,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Params {
    pub n: u32,
    pub r: f32, // radius of the magnets from centre
    pub d: f32,
    pub mu: f32, // coefficient of friction -> controls the complexity of the fractal (lower = more fancy)
    pub c: f32,
    pub dt: f32,
    w: u32,
    h: u32,
    pub velocity_magnitude: f32, // magnitude of initial velocity
    pub velocity_angle: f32,     // angle offset for velocity direction (in radians)
    pub velocity_pattern: u32,   // 0=radial, 1=tangential, 2=uniform, 3=zero
    _padding: f32,               // padding to maintain 16-byte alignment
}

struct GPUSimResources {
    vertex_buffer: wgpu::Buffer,
    param_buffer: wgpu::Buffer,
    compute_pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    render_bg: wgpu::BindGroup,

    _output_tex: (wgpu::Texture, wgpu::TextureView),
}

#[derive(Debug, Clone, Copy)]
pub struct GPUSim {
    pub params: Params,
    _scale: f32,
    _width: u32,
    _height: u32,
}

impl GPUSim {
    pub fn create_particles(width: u32, height: u32, scale: f32, params: &Params) -> Vec<Particle> {
        (0..width * height)
            .map(|i| {
                let u = (vec2(
                    (i % width) as f32 / width as f32,
                    (i / width) as f32 / height as f32,
                ) - Vec2::splat(0.5))
                    * scale;

                let du = match params.velocity_pattern {
                    0 => {
                        // Radial pattern: velocity points away from center
                        if u.length() > 0.001 {
                            params.velocity_magnitude
                                * u.normalize()
                                    .rotate(Vec2::from_angle(params.velocity_angle))
                        } else {
                            Vec2::from_angle(params.velocity_angle) * params.velocity_magnitude
                        }
                    }
                    1 => {
                        // Tangential pattern: velocity perpendicular to position
                        if u.length() > 0.001 {
                            params.velocity_magnitude
                                * Vec2::new(-u.y, u.x)
                                    .normalize()
                                    .rotate(Vec2::from_angle(params.velocity_angle))
                        } else {
                            Vec2::from_angle(params.velocity_angle + PI / 2.0)
                                * params.velocity_magnitude
                        }
                    }
                    2 => {
                        // Uniform direction: all particles have same velocity direction
                        Vec2::from_angle(params.velocity_angle) * params.velocity_magnitude
                    }
                    _ => {
                        // Zero velocity
                        Vec2::ZERO
                    }
                };

                Particle { u, du }
            })
            .collect()
    }

    pub fn new(
        wgpu_render_state: &eframe::egui_wgpu::RenderState,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Self {
        let params = Params {
            n: 5,
            r: 3.0,
            d: 0.4,
            mu: 0.2,
            c: 0.2,
            w: width,
            h: height,
            dt: 0.006,
            velocity_magnitude: 4.0,
            velocity_angle: PI / 2.0,
            velocity_pattern: 1, // tangential by default
            _padding: 0.0,
        };

        let (device, target_format) = (&wgpu_render_state.device, wgpu_render_state.target_format);
        let particles = Self::create_particles(width, height, scale, &params);

        let param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particles"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let particle_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particles"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let colormap_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("colormap"),
            contents: bytemuck::cast_slice(&TWILIGHT_MAP),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Avaialable Buffers"),
            entries: &[
                // Simulation Parameters
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE
                        | wgpu::ShaderStages::VERTEX
                        | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                        min_binding_size: None,
                    },
                    count: None,
                },
                // The texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
            module: &shader_module,
            entry_point: Some("comp_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("magpen texture"),
            size: wgpu::Extent3d {
                width,
                height,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        let texview = tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some("magpen texture id"),
            ..Default::default()
        });
        let out_tex = (tex, texview);

        let render_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: &[&render_bg_layout],
                push_constant_ranges: &[],
            });
        let vb_layout = wgpu::VertexBufferLayout {
            array_stride: 2 * std::mem::size_of::<Vec2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0=>Float32x2, 1=>Float32x2],
        };
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[vb_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            multisample: wgpu::MultisampleState::default(),
            depth_stencil: None,
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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
                    resource: wgpu::BindingResource::TextureView(&out_tex.1),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
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
                        buffer: &param_buffer,
                        offset: 0,
                        // size: None,
                        size: NonZeroU64::new(std::mem::size_of::<Params>() as u64),
                    }),
                },
                // particles
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &particle_buf,
                        offset: 0,
                        size: NonZeroU64::new(
                            (particles.len() * std::mem::size_of::<Particle>()) as u64,
                        ),
                    }),
                },
                // Current Particle data
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&out_tex.1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &colormap_buf,
                        offset: 0,
                        size: NonZeroU64::new(
                            (TWILIGHT_MAP.len() * std::mem::size_of::<[f32; 4]>()) as u64,
                        ),
                    }),
                },
            ],
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[
                [vec2(-1.0, -1.0), vec2(0.0, 0.0)],
                [vec2(1.0, -1.0), vec2(1.0, 0.0)],
                [vec2(-1.0, 1.0), vec2(0.0, 1.0)],
                [vec2(1.0, 1.0), vec2(1.0, 1.0)],
            ]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(GPUSimResources {
                bind_group,
                param_buffer,
                compute_pipeline,
                render_bg,
                render_pipeline,
                vertex_buffer,
                _output_tex: out_tex,
            });

        GPUSim {
            params,
            _scale: scale,
            _width: width,
            _height: height,
        }
    }

    pub fn restart(&mut self, wgpu_render_state: &eframe::egui_wgpu::RenderState) {
        let particles =
            Self::create_particles(self._width, self._height, self._scale, &self.params);
        let device = &wgpu_render_state.device;

        // Get current resources and recreate particle buffer
        if let Some(resources) = wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .get_mut::<GPUSimResources>()
        {
            // Create new particle buffer with reset particles
            let new_particle_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("particles"),
                contents: bytemuck::cast_slice(&particles),
                usage: wgpu::BufferUsages::STORAGE,
            });

            // Recreate the bind group with the new particle buffer
            let colormap_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("colormap"),
                contents: bytemuck::cast_slice(&TWILIGHT_MAP),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Avaialable Buffers"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE
                            | wgpu::ShaderStages::VERTEX
                            | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            let new_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bg_layout,
                label: Some("Resources described by the bind_group_layout"),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &resources.param_buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<Params>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &new_particle_buf,
                            offset: 0,
                            size: NonZeroU64::new(
                                (particles.len() * std::mem::size_of::<Particle>()) as u64,
                            ),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&resources._output_tex.1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &colormap_buf,
                            offset: 0,
                            size: NonZeroU64::new(
                                (TWILIGHT_MAP.len() * std::mem::size_of::<[f32; 4]>()) as u64,
                            ),
                        }),
                    },
                ],
            });

            resources.bind_group = new_bind_group;
        }
    }
}

impl eframe::egui_wgpu::CallbackTrait for GPUSim {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let res: &GPUSimResources = callback_resources.get().unwrap();
        queue.write_buffer(&res.param_buffer, 0, bytemuck::cast_slice(&[self.params]));
        let mut encoder = device.create_command_encoder(&Default::default());

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&res.compute_pipeline);
            cpass.set_bind_group(0, &res.bind_group, &[]);

            cpass.dispatch_workgroups(self.params.w, self.params.h, 1);
        }

        vec![encoder.finish()]
    }

    fn paint<'a, 'b, 'c>(
        &'a self,
        _info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'b>,
        callback_resources: &'c eframe::egui_wgpu::CallbackResources,
    ) {
        let res: &GPUSimResources = callback_resources.get().unwrap();

        render_pass.set_pipeline(&res.render_pipeline);
        render_pass.set_vertex_buffer(0, res.vertex_buffer.slice(..));
        render_pass.set_bind_group(0, &res.render_bg, &[]);
        render_pass.draw(0..4, 0..1);
    }
}
