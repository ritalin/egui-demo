use egui::epaint::Vertex;
use wgpu::{SurfaceTargetUnsafe, rwh::{HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle}};

mod buffer;

pub struct RawWindow {
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
}
impl RawWindow {
    pub fn create<T: HasDisplayHandle + HasWindowHandle + 'static>(target: &T) -> Result<Self, HandleError> {
        Ok(RawWindow {
            display_handle: target.display_handle()?.as_raw(),
            window_handle: target.window_handle()?.as_raw(),
        })
    }
}
impl From<&RawWindow> for SurfaceTargetUnsafe {
    fn from(value: &RawWindow) -> Self {
        Self::RawHandle{ raw_display_handle: value.display_handle, raw_window_handle: value.window_handle }
    }
}

pub struct ScreenDescriptor {
    pub pixel_per_point: f32,
    pub screen_width: u32,
    pub screen_height: u32,
}

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    texture_layout: wgpu::BindGroupLayout,
    texture_fallback: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bg_pipeline: wgpu::RenderPipeline,
    fg_pipeline: wgpu::RenderPipeline,
}
impl WgpuRenderer {
    pub async fn create(frame_width: u32, framw_height: u32, target: &RawWindow) -> Result<Self, anyhow::Error> {
        assert!(frame_width > 0 && framw_height > 0, "wgpu does nou allow size 0.");

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = unsafe { instance.create_surface_unsafe(target.into())? };
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await?;

        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        }).await?;

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps.formats.iter()
            .find(|fmt| fmt.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0])
        ;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: frame_width,
            height: framw_height,
            present_mode: caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,

                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,

                },
            ]
        });

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Default texture sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let buffer_fallback = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture image fallback"),
            size: wgpu::Extent3d{ width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[ wgpu::TextureFormat::Rgba8Unorm ],
        });
        let texture_fallback = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group fallback"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&buffer_fallback.create_view(&wgpu::TextureViewDescriptor::default())),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
            ],
        });

        let vertex_buffer = buffer::make_vertex_buffer(&device, size_of::<Vertex>() as u64 * 1024);
        let index_buffer = buffer::make_index_buffer(&device, size_of::<u32>() as u64 * 1024 * 3);

        let bg_pipeline = make_background_pipeline(&device, &config);
        let fg_pipeline = make_freground_pipeline(&device, &config, &[&texture_layout]);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            texture_layout,
            texture_fallback,
            vertex_buffer,
            index_buffer,
            bg_pipeline,
            fg_pipeline,
        })
    }

    pub fn request_resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(
        &mut self,
        screen: &ScreenDescriptor,
        triangles: &[egui::ClippedPrimitive]) -> Result<(), wgpu::SurfaceError>
    {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render encoder"),
        });

        let texture = self.surface.get_current_texture()?;
        let texture_view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        encode_bg(&mut encoder, &texture_view, &self.bg_pipeline);

        // buffer::send_texture();

        let (vbuffer_size, ibuffer_size) = buffer::measure_buffer_size(triangles);
        if (vbuffer_size > 0) && (ibuffer_size > 0) {
            buffer::send_vertex_buffer(&mut self.device, &self.queue, vbuffer_size, triangles, &mut self.vertex_buffer);
            buffer::send_index_buffer(&mut self.device, &self.queue, ibuffer_size, triangles, &mut self.index_buffer);
            encode_fg(&mut encoder, &texture_view, &self.fg_pipeline, &self.vertex_buffer, &self.index_buffer, &self.texture_fallback, screen, triangles);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        texture.present();
        Ok(())
    }
}

fn make_background_pipeline(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("bg_shader.wgsl"));
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render background pipline layout"),
        bind_group_layouts: &[],
        immediate_size: 0
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
        label: Some("Render background pipline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default()
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState { count: 1, mask: 0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module:&shader,
            entry_point: Some("fs_main"),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })
            ],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn make_freground_pipeline(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, bindgroups: &[&wgpu::BindGroupLayout]) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("egui.wgsl"));
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render widget pipline layout"),
        bind_group_layouts: bindgroups,
        immediate_size: 0
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
        label: Some("Render background pipline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                }
            ],
            compilation_options: wgpu::PipelineCompilationOptions::default()
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode:  None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })
            ],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn encode_bg(encoder: &mut wgpu::CommandEncoder, texture_view: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render background pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color{ r: 0.1, g: 0.2, b: 0.3, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })
        ],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    pass.set_pipeline(pipeline);
    pass.draw(0..3, 0..1);
}

fn encode_fg(
    encoder: &mut wgpu::CommandEncoder,
    texture_view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    vertex_buffer: &wgpu::Buffer,
    index_buffer: &wgpu::Buffer,
    bind_group_fallback: &wgpu::BindGroup,
    screen: &ScreenDescriptor,
    triangles: &[egui::ClippedPrimitive])
{
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render mesh pass"),
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment {
                view: texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
        ],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    pass.set_pipeline(pipeline);

    let mut voffset = 0;
    let mut ioffset = 0;

    pass.set_bind_group(0, bind_group_fallback, &[]);

    for egui::ClippedPrimitive{ clip_rect, primitive } in triangles {
        let Some((x, y, width, height)) = to_scissor_rect(clip_rect, &screen) else { continue };
        pass.set_scissor_rect(x, y, width, height);

        match primitive {
            egui::epaint::Primitive::Mesh(egui::Mesh{ indices, vertices, .. }) => {
                let vrange = voffset..voffset + (vertices.len() * size_of::<Vertex>()) as u64;
                let irange = ioffset..ioffset + (indices.len() * size_of::<u32>()) as u64;

                voffset = vrange.end;
                ioffset = irange.end;

                pass.set_vertex_buffer(0, vertex_buffer.slice(vrange));
                pass.set_index_buffer(index_buffer.slice(irange), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
            egui::epaint::Primitive::Callback(_paint_callback) => {
                panic!("Not implemented");
            }
        }
    }
}

fn to_scissor_rect(clip_rect: &egui::Rect, &ScreenDescriptor{ pixel_per_point: ppp, screen_width, screen_height }: &ScreenDescriptor) -> Option<(u32, u32, u32, u32)> {
    let x0 = (clip_rect.left() * ppp).round() as u32;
    let y0 = (clip_rect.top() * ppp).round() as u32;
    let x1 = (clip_rect.right() * ppp).round() as u32;
    let y1 = (clip_rect.bottom() * ppp).round() as u32;

    let x = x0.clamp(0, screen_width);
    let y = y0.clamp(0, screen_height);
    let w = u32::saturating_sub(x1.clamp(0, screen_width), x);
    let h = u32::saturating_sub(y1.clamp(0, screen_height), y);

    ((w != 0) && (h != 0)).then(|| (x, y, w, h))
}
