pub mod object;

use std::sync::Arc;

use egui_wgpu::Renderer;
use winit::window::Window;

use crate::gui::Gui;

pub struct State<'a> {
    pub(crate) surface: wgpu::Surface<'a>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) size: winit::dpi::PhysicalSize<u32>,

    pub(crate) egui_renderer: Renderer,
    pub(crate) egui_winit_state: egui_winit::State,

    pub(crate) gui: Gui,

    // Needs to be declared after surface
    window: Arc<Window>,
}

impl<'a> State<'a> {
    pub(crate) async fn new(window: Arc<Window>) -> State<'a> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.expect("Failed to create device");

        let surface_capabilities = surface.get_capabilities(&adapter);
        let surface_format = surface_capabilities.formats.iter()
            .find(|format| {
                format.is_srgb()
            })
            .copied()
            .unwrap_or(surface_capabilities.formats[0]);

        let present_mode = surface_capabilities.present_modes.iter()
            .find(|mode| {
                **mode == wgpu::PresentMode::Fifo
            })
            .copied()
            .unwrap_or(surface_capabilities.present_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: Default::default(),
            desired_maximum_frame_latency: 2,
        };

        let egui_ctx = egui::Context::default();
        let egui_winit_state = {
            let mut egui_winit_state = egui_winit::State::new(
                egui_ctx,
                egui::ViewportId::ROOT,
                window.as_ref(),
                None,
                None
            );
            egui_winit_state.set_max_texture_side(device.limits().max_texture_dimension_2d as usize);
            egui_winit_state
        };

        let egui_renderer = Renderer::new(
            &device,
            config.format,
            None,
            1
        );

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            egui_renderer,
            egui_winit_state,
            gui: Gui::new(),
        }
    }

    pub fn window(&self) -> Arc<Window> {
        self.window.clone()
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn update(&mut self) {
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        let gui_buffers = {
            let raw_input = self.egui_winit_state.take_egui_input(self.window.as_ref());
            let full_output = self.egui_winit_state.egui_ctx().run(raw_input, |context| {
                // Draw your UI here
                self.gui.render(context)
            });
            self.egui_winit_state.handle_platform_output(self.window.as_ref(), full_output.platform_output);
                
            let clipped_primitives = self
                .egui_winit_state
                .egui_ctx()
                .tessellate(full_output.shapes, full_output.pixels_per_point);

            let screen_descriptors = egui_wgpu::ScreenDescriptor {
                size_in_pixels: self.size.into(),
                pixels_per_point: full_output.pixels_per_point
            };

            for (id, image_delta) in &full_output.textures_delta.set {
                self.egui_renderer.update_texture(
                    &self.device,
                    &self.queue,
                    *id,
                    image_delta,
                );
            }

            let command_buffers = self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptors
            );


            {
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                self.egui_renderer.render(
                    &mut render_pass,
                    &clipped_primitives,
                    &screen_descriptors, 
                );
            }

            for id in &full_output.textures_delta.free {
                self.egui_renderer.free_texture(id);
            }

            command_buffers
        };

        let mut command_buffers = vec![];
        command_buffers.extend(gui_buffers);
        command_buffers.push(encoder.finish());

        self.queue.submit(command_buffers);
        output.present();

        Ok(())
    }
}
