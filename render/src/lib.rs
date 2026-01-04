use std::collections::HashMap;

use fontdue::Font;
use wgpu::util::DeviceExt;

pub const CELL_WIDTH: u32 = 10;
pub const CELL_HEIGHT: u32 = 20;
pub const PADDING_X: u32 = 12;
pub const PADDING_Y: u32 = 12;
pub const DEFAULT_FONT_SIZE: f32 = 16.0;

const COLOR_BG: [u8; 4] = [10, 14, 20, 255];
const COLOR_FG: [u8; 4] = [230, 237, 243, 255];
const COLOR_CURSOR: [u8; 4] = [88, 168, 255, 255];

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("invalid surface size: {width}x{height}")]
    InvalidSize { width: u32, height: u32 },
    #[error("grid mismatch: expected {expected} cells, got {actual}")]
    GridMismatch { expected: usize, actual: usize },
    #[error("font error: {0}")]
    Font(String),
    #[error("surface error: {0}")]
    Surface(#[from] wgpu::SurfaceError),
}

#[derive(Debug, Copy, Clone)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

pub struct RenderGrid<'a> {
    pub cols: u16,
    pub rows: u16,
    pub cells: &'a [char],
    pub cursor: Option<CursorPosition>,
    pub cursor_visible: bool,
}

#[derive(Debug, Copy, Clone)]
pub struct CursorPosition {
    pub col: u16,
    pub row: u16,
}

pub struct FontSpec {
    pub bytes: Vec<u8>,
    pub size: f32,
}

pub struct Renderer<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    pixel_buffer: Vec<u8>,
    texture_size: RenderSize,
    row_stride: u32,
    font: FontRasterizer,
}

impl<'a> Renderer<'a> {
    pub fn new(
        surface: wgpu::Surface<'a>,
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        size: RenderSize,
        font: FontSpec,
    ) -> Result<Self, RenderError> {
        let config = configure_surface(&surface, adapter, size)?;
        surface.configure(&device, &config);

        let font = FontRasterizer::new(font)?;

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render_bind_group_layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render_shader"),
            source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render_vertex_buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let texture_size = RenderSize {
            width: config.width,
            height: config.height,
        };

        let (texture, texture_view, sampler, bind_group, pixel_buffer, row_stride) =
            create_texture_resources(&device, &bind_group_layout, texture_size);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group_layout,
            bind_group,
            texture,
            texture_view,
            sampler,
            vertex_buffer,
            pixel_buffer,
            texture_size,
            row_stride,
            font,
        })
    }

    pub fn resize(&mut self, size: RenderSize) -> Result<(), RenderError> {
        self.config.width = size.width;
        self.config.height = size.height;
        if size.width == 0 || size.height == 0 {
            return Err(RenderError::InvalidSize {
                width: size.width,
                height: size.height,
            });
        }
        self.surface.configure(&self.device, &self.config);
        let (texture, texture_view, sampler, bind_group, pixel_buffer, row_stride) =
            create_texture_resources(&self.device, &self.bind_group_layout, size);
        self.texture = texture;
        self.texture_view = texture_view;
        self.sampler = sampler;
        self.bind_group = bind_group;
        self.pixel_buffer = pixel_buffer;
        self.texture_size = size;
        self.row_stride = row_stride;
        Ok(())
    }

    pub fn set_font(&mut self, font: FontSpec) -> Result<(), RenderError> {
        self.font = FontRasterizer::new(font)?;
        Ok(())
    }

    pub fn render(&mut self, grid: &RenderGrid<'_>) -> Result<(), RenderError> {
        self.update_pixels(grid)?;
        self.upload_texture();

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color_to_wgpu(COLOR_BG)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..VERTICES.len() as u32, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }

    fn update_pixels(&mut self, grid: &RenderGrid<'_>) -> Result<(), RenderError> {
        let expected = grid.cols as usize * grid.rows as usize;
        if grid.cells.len() != expected {
            return Err(RenderError::GridMismatch {
                expected,
                actual: grid.cells.len(),
            });
        }

        fill_background(&mut self.pixel_buffer, self.row_stride as usize, COLOR_BG);

        let usable_width = self.texture_size.width.saturating_sub(PADDING_X * 2);
        let usable_height = self.texture_size.height.saturating_sub(PADDING_Y * 2);
        let max_cols = (usable_width / CELL_WIDTH) as usize;
        let max_rows = (usable_height / CELL_HEIGHT) as usize;
        let cols = grid.cols.min(max_cols as u16) as usize;
        let rows = grid.rows.min(max_rows as u16) as usize;

        for row in 0..rows {
            for col in 0..cols {
                let idx = row * grid.cols as usize + col;
                let ch = grid.cells[idx];
                let draw = DrawContext {
                    font: &mut self.font,
                    ch,
                    origin_x: PADDING_X + col as u32 * CELL_WIDTH,
                    origin_y: PADDING_Y + row as u32 * CELL_HEIGHT,
                    width: self.texture_size.width as usize,
                    height: self.texture_size.height as usize,
                    stride: self.row_stride as usize,
                    buffer: &mut self.pixel_buffer,
                };
                draw_glyph(draw);
            }
        }

        if grid.cursor_visible {
            if let Some(cursor) = grid.cursor {
                if cursor.col < grid.cols && cursor.row < grid.rows {
                    let cursor_x = PADDING_X + cursor.col as u32 * CELL_WIDTH;
                    let cursor_y = PADDING_Y + cursor.row as u32 * CELL_HEIGHT;
                    draw_cursor_bar(
                        cursor_x,
                        cursor_y,
                        self.texture_size.width as usize,
                        self.texture_size.height as usize,
                        self.row_stride as usize,
                        &mut self.pixel_buffer,
                    );
                }
            }
        }

        Ok(())
    }

    fn upload_texture(&self) {
        let width = self.texture_size.width;
        let height = self.texture_size.height;
        let bytes_per_row = Some(self.row_stride);
        let rows_per_image = Some(height);

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.pixel_buffer,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row,
                rows_per_image,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}

fn configure_surface(
    surface: &wgpu::Surface,
    adapter: &wgpu::Adapter,
    size: RenderSize,
) -> Result<wgpu::SurfaceConfiguration, RenderError> {
    if size.width == 0 || size.height == 0 {
        return Err(RenderError::InvalidSize {
            width: size.width,
            height: size.height,
        });
    }

    let capabilities = surface.get_capabilities(adapter);
    let format = capabilities
        .formats
        .first()
        .copied()
        .ok_or(RenderError::InvalidSize {
            width: size.width,
            height: size.height,
        })?;
    let present_mode =
        capabilities
            .present_modes
            .first()
            .copied()
            .ok_or(RenderError::InvalidSize {
                width: size.width,
                height: size.height,
            })?;
    let alpha_mode = capabilities
        .alpha_modes
        .first()
        .copied()
        .ok_or(RenderError::InvalidSize {
            width: size.width,
            height: size.height,
        })?;

    Ok(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode,
        alpha_mode,
        view_formats: Vec::new(),
        desired_maximum_frame_latency: 2,
    })
}

fn create_texture_resources(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    size: RenderSize,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Sampler,
    wgpu::BindGroup,
    Vec<u8>,
    u32,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render_text_texture"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("render_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let row_stride = aligned_row_bytes(size.width);
    let pixel_buffer = vec![0u8; (row_stride * size.height) as usize];

    (
        texture,
        texture_view,
        sampler,
        bind_group,
        pixel_buffer,
        row_stride,
    )
}

fn aligned_row_bytes(width: u32) -> u32 {
    let bytes_per_pixel = 4;
    let row_bytes = width * bytes_per_pixel;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padding = (alignment - (row_bytes % alignment)) % alignment;
    row_bytes + padding
}

fn fill_background(buffer: &mut [u8], stride: usize, color: [u8; 4]) {
    for row in buffer.chunks_exact_mut(stride) {
        for pixel in row.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
    }
}

struct DrawContext<'a> {
    font: &'a mut FontRasterizer,
    ch: char,
    origin_x: u32,
    origin_y: u32,
    width: usize,
    height: usize,
    stride: usize,
    buffer: &'a mut [u8],
}

fn draw_glyph(ctx: DrawContext<'_>) {
    let line_metrics = ctx.font.line_metrics;
    let glyph = match ctx.font.rasterize(ctx.ch) {
        Some(glyph) => glyph,
        None => return,
    };

    if glyph.width == 0 || glyph.height == 0 {
        return;
    }

    let cell_w = CELL_WIDTH as f32;
    let cell_h = CELL_HEIGHT as f32;
    let mut base_x = ctx.origin_x as f32;
    if glyph.advance_width > 0.0 {
        let padding = (cell_w - glyph.advance_width).max(0.0) * 0.5;
        base_x += padding;
    }

    let base_y = if let Some(metrics) = line_metrics {
        let line_height = metrics.ascent - metrics.descent;
        let padding = (cell_h - line_height).max(0.0) * 0.5;
        ctx.origin_y as f32 + padding + metrics.ascent
    } else {
        ctx.origin_y as f32 + cell_h * 0.8
    };

    let base_x = (base_x + glyph.xmin as f32).round() as i32;
    let base_y = (base_y - (glyph.ymin as f32 + glyph.height as f32)).round() as i32;

    for y in 0..glyph.height {
        for x in 0..glyph.width {
            let alpha = glyph.data[(y * glyph.width + x) as usize];
            if alpha == 0 {
                continue;
            }
            let px = base_x + x as i32;
            let py = base_y + y as i32;
            if px < 0 || py < 0 {
                continue;
            }
            let px = px as usize;
            let py = py as usize;
            if px >= ctx.width || py >= ctx.height {
                continue;
            }
            let idx = py * ctx.stride + px * 4;
            if idx + 4 <= ctx.buffer.len() {
                blend_pixel(&mut ctx.buffer[idx..idx + 4], COLOR_FG, alpha);
            }
        }
    }
}

fn draw_cursor_bar(
    origin_x: u32,
    origin_y: u32,
    width: usize,
    height: usize,
    stride: usize,
    buffer: &mut [u8],
) {
    let bar_width = 2u32;
    let bar_height = CELL_HEIGHT.saturating_sub(4);
    let start_x = origin_x + 1;
    let start_y = origin_y + 2;

    for y in 0..bar_height {
        let py = start_y + y;
        if py as usize >= height {
            continue;
        }
        for x in 0..bar_width {
            let px = start_x + x;
            if px as usize >= width {
                continue;
            }
            let idx = py as usize * stride + px as usize * 4;
            if idx + 4 <= buffer.len() {
                buffer[idx..idx + 4].copy_from_slice(&COLOR_CURSOR);
            }
        }
    }
}

fn blend_pixel(dst: &mut [u8], fg: [u8; 4], alpha: u8) {
    let a = alpha as u32;
    let inv = 255 - alpha as u32;
    dst[0] = ((fg[0] as u32 * a + dst[0] as u32 * inv) / 255) as u8;
    dst[1] = ((fg[1] as u32 * a + dst[1] as u32 * inv) / 255) as u8;
    dst[2] = ((fg[2] as u32 * a + dst[2] as u32 * inv) / 255) as u8;
    dst[3] = 255;
}

fn color_to_wgpu(color: [u8; 4]) -> wgpu::Color {
    wgpu::Color {
        r: color[0] as f64 / 255.0,
        g: color[1] as f64 / 255.0,
        b: color[2] as f64 / 255.0,
        a: color[3] as f64 / 255.0,
    }
}

struct FontRasterizer {
    font: Font,
    size: f32,
    cache: HashMap<char, GlyphBitmap>,
    line_metrics: Option<fontdue::LineMetrics>,
}

impl FontRasterizer {
    fn new(spec: FontSpec) -> Result<Self, RenderError> {
        let font = Font::from_bytes(spec.bytes, fontdue::FontSettings::default())
            .map_err(|err| RenderError::Font(err.to_string()))?;
        let line_metrics = font.horizontal_line_metrics(spec.size);
        Ok(Self {
            font,
            size: spec.size,
            cache: HashMap::new(),
            line_metrics,
        })
    }

    fn rasterize(&mut self, ch: char) -> Option<&GlyphBitmap> {
        if !self.cache.contains_key(&ch) {
            let (metrics, bitmap) = self.font.rasterize(ch, self.size);
            let glyph = GlyphBitmap {
                width: metrics.width as u32,
                height: metrics.height as u32,
                xmin: metrics.xmin,
                ymin: metrics.ymin,
                advance_width: metrics.advance_width,
                data: bitmap,
            };
            self.cache.insert(ch, glyph);
        }
        self.cache.get(&ch)
    }
}

struct GlyphBitmap {
    width: u32,
    height: u32,
    xmin: i32,
    ymin: i32,
    advance_width: f32,
    data: Vec<u8>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0],
        uv: [0.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, -1.0],
        uv: [0.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0],
        uv: [0.0, 0.0],
    },
];

const RENDER_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0)
var screen_texture: texture_2d<f32>;
@group(0) @binding(1)
var screen_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#;
