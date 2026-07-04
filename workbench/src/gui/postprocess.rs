//! Centered blit post-processor (issue #28). The upstream default stretches
//! the grid-sized text texture across the whole surface, distorting glyphs
//! whenever the window is not an exact cell multiple. This one blits 1:1 -
//! pixel-exact glyphs - centered in the surface, and fills the margin with
//! the theme background, which doubles as the window padding (combined with
//! `Viewport::Shrink` reserving a minimum inset).
//!
//! Pipeline skeleton adapted from ratatui-wgpu's `DefaultPostProcessor`
//! (MIT OR Apache-2.0).

use std::mem::size_of;
use std::num::NonZeroU64;

use ratatui_wgpu::PostProcessor;
use wgpu::{
    include_wgsl, AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoder, Device, FilterMode, FragmentState, LoadOp, MultisampleState, Operations,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StoreOp, SurfaceConfiguration,
    TextureSampleType, TextureView, TextureViewDimension, VertexState,
};

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Debug, Clone, Copy)]
struct Uniforms {
    screen_size: [f32; 2],
    use_srgb: u32,
    _pad: u32,
    bg: [f32; 4],
}

/// sRGB background color (0..1 per channel) painted in the margins.
pub type BgColor = [f32; 4];

pub struct CenteredPostProcessor {
    bg: BgColor,
    uniforms: Buffer,
    bindings: BindGroupLayout,
    sampler: Sampler,
    pipeline: RenderPipeline,
    bind_group: BindGroup,
}

impl PostProcessor for CenteredPostProcessor {
    type UserData = BgColor;

    fn compile(
        device: &Device,
        text_view: &TextureView,
        surface_config: &SurfaceConfiguration,
        bg: Self::UserData,
    ) -> Self {
        let uniforms = device.create_buffer(&BufferDescriptor {
            label: Some("Centered Blit Uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let bindings = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Centered Blit Bindings Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<Uniforms>() as u64),
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(include_wgsl!("shaders/centered_blit.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Centered Blit Layout"),
            bind_group_layouts: &[&bindings],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Centered Blit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: surface_config.format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let bind_group = build_bind_group(device, &bindings, text_view, &sampler, &uniforms);

        Self {
            bg,
            uniforms,
            bindings,
            sampler,
            pipeline,
            bind_group,
        }
    }

    fn resize(
        &mut self,
        device: &Device,
        text_view: &TextureView,
        _surface_config: &SurfaceConfiguration,
    ) {
        self.bind_group = build_bind_group(
            device,
            &self.bindings,
            text_view,
            &self.sampler,
            &self.uniforms,
        );
    }

    fn process(
        &mut self,
        encoder: &mut CommandEncoder,
        queue: &Queue,
        _text_view: &TextureView,
        surface_config: &SurfaceConfiguration,
        surface_view: &TextureView,
    ) {
        {
            let mut uniforms = queue
                .write_buffer_with(
                    &self.uniforms,
                    0,
                    NonZeroU64::new(size_of::<Uniforms>() as u64).unwrap(),
                )
                .unwrap();
            uniforms.copy_from_slice(bytemuck::bytes_of(&Uniforms {
                screen_size: [surface_config.width as f32, surface_config.height as f32],
                use_srgb: u32::from(surface_config.format.is_srgb()),
                _pad: 0,
                bg: self.bg,
            }));
        }

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Centered Blit Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::TRANSPARENT),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn build_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    text_view: &TextureView,
    sampler: &Sampler,
    uniforms: &Buffer,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("Centered Blit Bindings"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(text_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
            BindGroupEntry {
                binding: 2,
                resource: uniforms.as_entire_binding(),
            },
        ],
    })
}
