//! WebGL2 renderer (through `glow`) with a PSX-style pipeline:
//! the scene is drawn into a low-resolution target with vertex snapping,
//! affine texture mapping, Gouraud lighting, per-vertex fog and 15-bit
//! dithered colour, then upscaled with nearest filtering.

mod shaders;

pub use shaders::MAX_LIGHTS;

use crate::math::{Mat4, Vec2, Vec3, Vec4};
use crate::mesh::{MeshData, Vertex};
use crate::texture::{TEX_SIZE, TextureData};
use glow::HasContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshHandle(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    pub pos: Vec3,
    pub radius: f32,
    pub color: Vec3,
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Blend {
    #[default]
    Opaque,
    Alpha,
    Additive,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawItem {
    pub mesh: MeshHandle,
    pub model: Mat4,
    pub tint: Vec4,
    /// Colour the surface is pushed towards (rgb) by amount (a), e.g. hit flashes.
    pub flash: Vec4,
    pub blend: Blend,
    pub double_sided: bool,
    pub uv_offset: Vec2,
    /// 0 = solid, 1 = fully dissolved (ordered-dither discard).
    pub dissolve: f32,
    /// Bounding sphere in world space, used for light selection and sorting.
    pub center: Vec3,
    pub radius: f32,
}

impl DrawItem {
    pub fn new(mesh: MeshHandle, model: Mat4, center: Vec3, radius: f32) -> Self {
        DrawItem {
            mesh,
            model,
            tint: Vec4::ONE,
            flash: Vec4::ZERO,
            blend: Blend::Opaque,
            double_sided: false,
            uv_offset: Vec2::ZERO,
            dissolve: 0.0,
            center,
            radius,
        }
    }
}

/// A camera-independent textured quad generated every frame (particles,
/// blob shadows, decals). Always drawn unlit.
#[derive(Clone, Copy, Debug)]
pub struct DynQuad {
    pub p: [Vec3; 4],
    pub uv: [Vec2; 4],
    pub color: [u8; 4],
    pub layer: u8,
    pub blend: Blend,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view: Mat4,
    pub proj: Mat4,
    pub pos: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub ambient: Vec3,
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub fog_color: Vec3,
    pub fog_near: f32,
    pub fog_far: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Environment {
            ambient: Vec3::splat(0.3),
            sun_dir: Vec3::new(0.3, -1.0, 0.2).normalize(),
            sun_color: Vec3::splat(0.5),
            fog_color: Vec3::splat(0.1),
            fog_near: 10.0,
            fog_far: 40.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderSettings {
    /// Height of the internal framebuffer in pixels; width follows the aspect.
    pub vertical_res: u32,
    pub vertex_snap: bool,
    pub affine: bool,
    pub dither: bool,
    /// Letterbox to 4:3 like a CRT TV.
    pub force_4_3: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings { vertical_res: 240, vertex_snap: true, affine: true, dither: true, force_4_3: false }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RenderList {
    pub camera: Option<Camera>,
    pub env: Environment,
    pub lights: Vec<PointLight>,
    /// Drawn first, without depth, following the camera.
    pub sky: Vec<DrawItem>,
    pub draws: Vec<DrawItem>,
    pub quads: Vec<DynQuad>,
    /// Full-screen fade colour (rgb) and amount (a).
    pub fade: Vec4,
}

impl RenderList {
    pub fn clear(&mut self) {
        self.camera = None;
        self.lights.clear();
        self.sky.clear();
        self.draws.clear();
        self.quads.clear();
        self.fade = Vec4::ZERO;
    }
}

struct GpuMesh {
    vao: glow::VertexArray,
    _vbo: glow::Buffer,
    _ibo: glow::Buffer,
    count: i32,
}

struct WorldUniforms {
    model: Option<glow::UniformLocation>,
    view: Option<glow::UniformLocation>,
    proj: Option<glow::UniformLocation>,
    tint: Option<glow::UniformLocation>,
    res: Option<glow::UniformLocation>,
    snap: Option<glow::UniformLocation>,
    ambient: Option<glow::UniformLocation>,
    sun_dir: Option<glow::UniformLocation>,
    sun_color: Option<glow::UniformLocation>,
    lpos: Option<glow::UniformLocation>,
    lcol: Option<glow::UniformLocation>,
    nlights: Option<glow::UniformLocation>,
    fog: Option<glow::UniformLocation>,
    fog_color: Option<glow::UniformLocation>,
    uv_offset: Option<glow::UniformLocation>,
    tex: Option<glow::UniformLocation>,
    affine: Option<glow::UniformLocation>,
    dither: Option<glow::UniformLocation>,
    flash: Option<glow::UniformLocation>,
    dissolve: Option<glow::UniformLocation>,
    additive: Option<glow::UniformLocation>,
}

struct Target {
    fbo: glow::Framebuffer,
    color: glow::Texture,
    depth: glow::Renderbuffer,
    w: i32,
    h: i32,
}

pub struct Renderer {
    gl: glow::Context,
    world: glow::Program,
    wu: WorldUniforms,
    blit: glow::Program,
    blit_src: Option<glow::UniformLocation>,
    blit_fade: Option<glow::UniformLocation>,
    blit_rect: Option<glow::UniformLocation>,
    empty_vao: glow::VertexArray,
    meshes: Vec<GpuMesh>,
    tex_array: Option<glow::Texture>,
    target: Option<Target>,
    dyn_vao: glow::VertexArray,
    dyn_vbo: glow::Buffer,
    dyn_ibo: glow::Buffer,
    dyn_verts: Vec<Vertex>,
    dyn_idx: Vec<u32>,
    pub settings: RenderSettings,
    /// Number of draw calls in the last frame (debug overlay).
    pub stats_draws: u32,
    pub stats_tris: u32,
}

unsafe fn compile(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let prog = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let s = gl.create_shader(kind)?;
            gl.shader_source(s, src);
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) {
                return Err(format!("shader compile error: {}", gl.get_shader_info_log(s)));
            }
            gl.attach_shader(prog, s);
            shaders.push(s);
        }
        gl.link_program(prog);
        if !gl.get_program_link_status(prog) {
            return Err(format!("program link error: {}", gl.get_program_info_log(prog)));
        }
        for s in shaders {
            gl.detach_shader(prog, s);
            gl.delete_shader(s);
        }
        Ok(prog)
    }
}

unsafe fn setup_vertex_layout(gl: &glow::Context) {
    unsafe {
        let stride = std::mem::size_of::<Vertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, stride, 24);
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_f32(3, 4, glow::UNSIGNED_BYTE, false, stride, 32);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_f32(4, 4, glow::UNSIGNED_BYTE, false, stride, 36);
    }
}

impl Renderer {
    pub fn new(gl: glow::Context) -> Result<Self, String> {
        unsafe {
            let world = compile(&gl, shaders::WORLD_VS, shaders::WORLD_FS)?;
            let u = |n: &str| gl.get_uniform_location(world, n);
            let wu = WorldUniforms {
                model: u("u_model"),
                view: u("u_view"),
                proj: u("u_proj"),
                tint: u("u_tint"),
                res: u("u_res"),
                snap: u("u_snap"),
                ambient: u("u_ambient"),
                sun_dir: u("u_sun_dir"),
                sun_color: u("u_sun_color"),
                lpos: u("u_lpos"),
                lcol: u("u_lcol"),
                nlights: u("u_nlights"),
                fog: u("u_fog"),
                fog_color: u("u_fog_color"),
                uv_offset: u("u_uv_offset"),
                tex: u("u_tex"),
                affine: u("u_affine"),
                dither: u("u_dither"),
                flash: u("u_flash"),
                dissolve: u("u_dissolve"),
                additive: u("u_additive"),
            };
            let blit = compile(&gl, shaders::BLIT_VS, shaders::BLIT_FS)?;
            let blit_src = gl.get_uniform_location(blit, "u_src");
            let blit_fade = gl.get_uniform_location(blit, "u_fade");
            let blit_rect = gl.get_uniform_location(blit, "u_rect");
            let empty_vao = gl.create_vertex_array()?;

            let dyn_vao = gl.create_vertex_array()?;
            let dyn_vbo = gl.create_buffer()?;
            let dyn_ibo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(dyn_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(dyn_vbo));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(dyn_ibo));
            setup_vertex_layout(&gl);
            gl.bind_vertex_array(None);

            Ok(Renderer {
                gl,
                world,
                wu,
                blit,
                blit_src,
                blit_fade,
                blit_rect,
                empty_vao,
                meshes: Vec::new(),
                tex_array: None,
                target: None,
                dyn_vao,
                dyn_vbo,
                dyn_ibo,
                dyn_verts: Vec::new(),
                dyn_idx: Vec::new(),
                settings: RenderSettings::default(),
                stats_draws: 0,
                stats_tris: 0,
            })
        }
    }

    /// Uploads all textures as layers of one 2D array texture. Layer `i` of
    /// the array is `textures[i]`; all must be `TEX_SIZE` square.
    pub fn upload_textures(&mut self, textures: &[TextureData]) {
        let gl = &self.gl;
        let mut bytes = Vec::with_capacity(textures.len() * TEX_SIZE * TEX_SIZE * 4);
        for t in textures {
            assert_eq!((t.width, t.height), (TEX_SIZE, TEX_SIZE), "texture size");
            bytes.extend(t.bytes());
        }
        unsafe {
            if let Some(old) = self.tex_array.take() {
                gl.delete_texture(old);
            }
            let tex = gl.create_texture().expect("texture");
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(tex));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_3d(
                glow::TEXTURE_2D_ARRAY,
                0,
                glow::RGBA8 as i32,
                TEX_SIZE as i32,
                TEX_SIZE as i32,
                textures.len() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&bytes)),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
            self.tex_array = Some(tex);
        }
    }

    pub fn upload_mesh(&mut self, mesh: &MeshData) -> MeshHandle {
        let gl = &self.gl;
        unsafe {
            let vao = gl.create_vertex_array().expect("vao");
            let vbo = gl.create_buffer().expect("vbo");
            let ibo = gl.create_buffer().expect("ibo");
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, mesh.vertex_bytes(), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
            gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, mesh.index_bytes(), glow::STATIC_DRAW);
            setup_vertex_layout(gl);
            gl.bind_vertex_array(None);
            self.meshes.push(GpuMesh { vao, _vbo: vbo, _ibo: ibo, count: mesh.indices.len() as i32 });
        }
        MeshHandle(self.meshes.len() as u32 - 1)
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Internal resolution for a canvas size.
    pub fn internal_size(&self, canvas_w: u32, canvas_h: u32) -> (u32, u32) {
        let h = self.settings.vertical_res.max(120);
        let aspect = if self.settings.force_4_3 { 4.0 / 3.0 } else { canvas_w as f32 / canvas_h.max(1) as f32 };
        let w = ((h as f32 * aspect).round() as u32).max(1);
        (w, h)
    }

    fn ensure_target(&mut self, w: i32, h: i32) {
        if self.target.as_ref().is_some_and(|t| t.w == w && t.h == h) {
            return;
        }
        let gl = &self.gl;
        unsafe {
            if let Some(t) = self.target.take() {
                gl.delete_framebuffer(t.fbo);
                gl.delete_texture(t.color);
                gl.delete_renderbuffer(t.depth);
            }
            let color = gl.create_texture().expect("tex");
            gl.bind_texture(glow::TEXTURE_2D, Some(color));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                w,
                h,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            let depth = gl.create_renderbuffer().expect("rb");
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
            gl.renderbuffer_storage(glow::RENDERBUFFER, glow::DEPTH_COMPONENT24, w, h);
            let fbo = gl.create_framebuffer().expect("fbo");
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(color), 0);
            gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::RENDERBUFFER, Some(depth));
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.target = Some(Target { fbo, color, depth, w, h });
        }
    }

    /// Picks the most relevant lights for a bounding sphere.
    fn select_lights(lights: &[PointLight], center: Vec3, radius: f32, out: &mut Vec<(f32, usize)>) {
        out.clear();
        for (i, l) in lights.iter().enumerate() {
            let d = (l.pos - center).length();
            if d > l.radius + radius {
                continue;
            }
            let near = (d - radius).max(0.0);
            let score = l.intensity * (1.0 - near / l.radius).max(0.0);
            out.push((score, i));
        }
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(MAX_LIGHTS);
    }

    pub fn render(&mut self, list: &RenderList, canvas_w: u32, canvas_h: u32) {
        let (lw, lh) = self.internal_size(canvas_w, canvas_h);
        self.ensure_target(lw as i32, lh as i32);
        self.stats_draws = 0;
        self.stats_tris = 0;
        let settings = self.settings;
        let gl = &self.gl;
        let target = self.target.as_ref().unwrap();
        let env = list.env;
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.fbo));
            gl.viewport(0, 0, target.w, target.h);
            gl.clear_color(env.fog_color.x, env.fog_color.y, env.fog_color.z, 1.0);
            gl.depth_mask(true);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            if let (Some(cam), Some(tex)) = (list.camera, self.tex_array) {
                gl.use_program(Some(self.world));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(tex));
                let wu = &self.wu;
                gl.uniform_1_i32(wu.tex.as_ref(), 0);
                gl.uniform_matrix_4_f32_slice(wu.view.as_ref(), false, &cam.view.to_cols_array());
                gl.uniform_matrix_4_f32_slice(wu.proj.as_ref(), false, &cam.proj.to_cols_array());
                gl.uniform_2_f32(wu.res.as_ref(), target.w as f32, target.h as f32);
                gl.uniform_1_f32(wu.snap.as_ref(), if settings.vertex_snap { 1.0 } else { 0.0 });
                gl.uniform_1_f32(wu.affine.as_ref(), if settings.affine { 1.0 } else { 0.0 });
                gl.uniform_1_f32(wu.dither.as_ref(), if settings.dither { 1.0 } else { 0.0 });
                gl.uniform_3_f32_slice(wu.ambient.as_ref(), &env.ambient.to_array());
                gl.uniform_3_f32_slice(wu.sun_dir.as_ref(), &env.sun_dir.normalize_or(Vec3::NEG_Y).to_array());
                gl.uniform_3_f32_slice(wu.sun_color.as_ref(), &env.sun_color.to_array());
                gl.uniform_3_f32_slice(wu.fog_color.as_ref(), &env.fog_color.to_array());

                // Sky: no depth, no fog.
                gl.disable(glow::DEPTH_TEST);
                gl.depth_mask(false);
                gl.disable(glow::CULL_FACE);
                gl.disable(glow::BLEND);
                gl.uniform_2_f32(wu.fog.as_ref(), 1e6, 1e6 + 1.0);
                gl.uniform_1_i32(wu.nlights.as_ref(), 0);
                for d in &list.sky {
                    Self::draw_item(gl, wu, &self.meshes, d, &mut self.stats_draws, &mut self.stats_tris);
                }

                gl.uniform_2_f32(wu.fog.as_ref(), env.fog_near, env.fog_far);
                gl.enable(glow::DEPTH_TEST);
                gl.depth_func(glow::LEQUAL);
                gl.depth_mask(true);
                gl.enable(glow::CULL_FACE);
                gl.cull_face(glow::BACK);
                gl.front_face(glow::CCW);

                let mut sel = Vec::with_capacity(16);
                let mut lpos = [0f32; MAX_LIGHTS * 4];
                let mut lcol = [0f32; MAX_LIGHTS * 4];
                let mut translucent: Vec<&DrawItem> = Vec::new();
                for d in &list.draws {
                    if d.blend != Blend::Opaque {
                        translucent.push(d);
                        continue;
                    }
                    Self::bind_lights(gl, wu, &list.lights, d, &mut sel, &mut lpos, &mut lcol);
                    if d.double_sided {
                        gl.disable(glow::CULL_FACE);
                    }
                    Self::draw_item(gl, wu, &self.meshes, d, &mut self.stats_draws, &mut self.stats_tris);
                    if d.double_sided {
                        gl.enable(glow::CULL_FACE);
                    }
                }

                // Translucent meshes, far to near.
                translucent.sort_by(|a, b| {
                    let da = (a.center - cam.pos).length_squared();
                    let db = (b.center - cam.pos).length_squared();
                    db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
                });
                gl.enable(glow::BLEND);
                gl.depth_mask(false);
                for d in translucent {
                    match d.blend {
                        Blend::Additive => gl.blend_func(glow::SRC_ALPHA, glow::ONE),
                        _ => gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
                    }
                    gl.uniform_1_f32(wu.additive.as_ref(), if d.blend == Blend::Additive { 1.0 } else { 0.0 });
                    Self::bind_lights(gl, wu, &list.lights, d, &mut sel, &mut lpos, &mut lcol);
                    if d.double_sided {
                        gl.disable(glow::CULL_FACE);
                    }
                    Self::draw_item(gl, wu, &self.meshes, d, &mut self.stats_draws, &mut self.stats_tris);
                    gl.enable(glow::CULL_FACE);
                }
                gl.uniform_1_f32(wu.additive.as_ref(), 0.0);

                // Dynamic quads (particles, shadows): unlit vertex colour.
                if !list.quads.is_empty() {
                    gl.disable(glow::CULL_FACE);
                    gl.uniform_1_i32(wu.nlights.as_ref(), 0);
                    gl.uniform_matrix_4_f32_slice(wu.model.as_ref(), false, &Mat4::IDENTITY.to_cols_array());
                    gl.uniform_4_f32(wu.tint.as_ref(), 1.0, 1.0, 1.0, 1.0);
                    gl.uniform_4_f32(wu.flash.as_ref(), 0.0, 0.0, 0.0, 0.0);
                    gl.uniform_2_f32(wu.uv_offset.as_ref(), 0.0, 0.0);
                    gl.uniform_1_f32(wu.dissolve.as_ref(), 0.0);
                    for blend in [Blend::Alpha, Blend::Additive] {
                        self.dyn_verts.clear();
                        self.dyn_idx.clear();
                        for q in list.quads.iter().filter(|q| q.blend == blend) {
                            let n = (q.p[1] - q.p[0]).cross(q.p[2] - q.p[0]).normalize_or(Vec3::Y);
                            let base = self.dyn_verts.len() as u32;
                            for k in 0..4 {
                                self.dyn_verts.push(Vertex {
                                    pos: q.p[k].to_array(),
                                    normal: n.to_array(),
                                    uv: q.uv[k].to_array(),
                                    color: q.color,
                                    tex: [q.layer, crate::mesh::VF_UNLIT, 0, 0],
                                });
                            }
                            self.dyn_idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                        }
                        if self.dyn_idx.is_empty() {
                            continue;
                        }
                        match blend {
                            Blend::Additive => gl.blend_func(glow::SRC_ALPHA, glow::ONE),
                            _ => gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
                        }
                        gl.uniform_1_f32(wu.additive.as_ref(), if blend == Blend::Additive { 1.0 } else { 0.0 });
                        gl.bind_vertex_array(Some(self.dyn_vao));
                        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.dyn_vbo));
                        let vb = std::slice::from_raw_parts(
                            self.dyn_verts.as_ptr() as *const u8,
                            self.dyn_verts.len() * std::mem::size_of::<Vertex>(),
                        );
                        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vb, glow::STREAM_DRAW);
                        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.dyn_ibo));
                        let ib = std::slice::from_raw_parts(self.dyn_idx.as_ptr() as *const u8, self.dyn_idx.len() * 4);
                        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, ib, glow::STREAM_DRAW);
                        gl.draw_elements(glow::TRIANGLES, self.dyn_idx.len() as i32, glow::UNSIGNED_INT, 0);
                        self.stats_draws += 1;
                        self.stats_tris += self.dyn_idx.len() as u32 / 3;
                    }
                    gl.uniform_1_f32(wu.additive.as_ref(), 0.0);
                }
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.bind_vertex_array(None);
            }

            // Upscale to the canvas.
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(0, 0, canvas_w as i32, canvas_h as i32);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.use_program(Some(self.blit));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(target.color));
            gl.uniform_1_i32(self.blit_src.as_ref(), 0);
            gl.uniform_4_f32_slice(self.blit_fade.as_ref(), &list.fade.to_array());
            let rect = if settings.force_4_3 {
                let ca = canvas_w as f32 / canvas_h.max(1) as f32;
                let ta = 4.0 / 3.0;
                if ca > ta {
                    let w = ta / ca;
                    [(1.0 - w) * 0.5, 0.0, w, 1.0]
                } else {
                    let h = ca / ta;
                    [0.0, (1.0 - h) * 0.5, 1.0, h]
                }
            } else {
                [0.0, 0.0, 1.0, 1.0]
            };
            gl.uniform_4_f32_slice(self.blit_rect.as_ref(), &rect);
            gl.bind_vertex_array(Some(self.empty_vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn bind_lights(
        gl: &glow::Context,
        wu: &WorldUniforms,
        lights: &[PointLight],
        d: &DrawItem,
        sel: &mut Vec<(f32, usize)>,
        lpos: &mut [f32; MAX_LIGHTS * 4],
        lcol: &mut [f32; MAX_LIGHTS * 4],
    ) {
        Self::select_lights(lights, d.center, d.radius, sel);
        for (k, (_, i)) in sel.iter().enumerate() {
            let l = &lights[*i];
            lpos[k * 4..k * 4 + 4].copy_from_slice(&[l.pos.x, l.pos.y, l.pos.z, l.radius]);
            lcol[k * 4..k * 4 + 4].copy_from_slice(&[l.color.x, l.color.y, l.color.z, l.intensity]);
        }
        unsafe {
            gl.uniform_1_i32(wu.nlights.as_ref(), sel.len() as i32);
            if !sel.is_empty() {
                gl.uniform_4_f32_slice(wu.lpos.as_ref(), &lpos[..]);
                gl.uniform_4_f32_slice(wu.lcol.as_ref(), &lcol[..]);
            }
        }
    }

    unsafe fn draw_item(gl: &glow::Context, wu: &WorldUniforms, meshes: &[GpuMesh], d: &DrawItem, draws: &mut u32, tris: &mut u32) {
        let Some(m) = meshes.get(d.mesh.0 as usize) else { return };
        if m.count == 0 {
            return;
        }
        unsafe {
            gl.uniform_matrix_4_f32_slice(wu.model.as_ref(), false, &d.model.to_cols_array());
            gl.uniform_4_f32_slice(wu.tint.as_ref(), &d.tint.to_array());
            gl.uniform_4_f32_slice(wu.flash.as_ref(), &d.flash.to_array());
            gl.uniform_2_f32(wu.uv_offset.as_ref(), d.uv_offset.x, d.uv_offset.y);
            gl.uniform_1_f32(wu.dissolve.as_ref(), d.dissolve);
            gl.bind_vertex_array(Some(m.vao));
            gl.draw_elements(glow::TRIANGLES, m.count, glow::UNSIGNED_INT, 0);
        }
        *draws += 1;
        *tris += m.count as u32 / 3;
    }
}
