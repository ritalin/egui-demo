use wgpu::{SurfaceTargetUnsafe, rwh::{HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle}};

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

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_dirty: bool,
    render_pipeline: wgpu::RenderPipeline,
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render pipline layout"),
            bind_group_layouts: &[],
            immediate_size: 0
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label: Some("Render pipline"),
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
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_dirty: false,
            render_pipeline,
        })
    }

    pub fn request_resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.is_dirty = true;
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if !self.is_dirty {
            return Ok(())
        }

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &view,
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
            pass.set_pipeline(&self.render_pipeline);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
