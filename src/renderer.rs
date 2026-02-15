use std::rc::Rc;
use anyhow::{anyhow, bail};
use imgui_glow_renderer::glow;
use imgui_glow_renderer::glow::{HasContext, NativeProgram, NativeShader, NativeVertexArray};
use anyhow::Result;

pub struct VeilDERenderer {
    gl: Rc<glow::Context>,
    program: NativeProgram,
    vertex_array: NativeVertexArray,
}

impl VeilDERenderer {
    pub fn new(gl: &Rc<glow::Context>) -> Result<Self> {
        unsafe {
            let program = gl
                .create_program()
                .map_err(|_| anyhow!("Failed to create OpenGL program"))?;

            let vertex_array = gl
                .create_vertex_array()
                .map_err(|_| anyhow!("Failed to create vertex array"))?;

            let mut shaders = [
                (glow::VERTEX_SHADER, crate::consts::VERTEX_SHADER_SOURCE, Option::<NativeShader>::None),
                (glow::FRAGMENT_SHADER, crate::consts::FRAGMENT_SHADER_SOURCE, Option::<NativeShader>::None)
            ];

            for (kind, source, handle) in shaders.iter_mut() {
                let shader = gl
                    .create_shader(*kind)
                    .map_err(|_| anyhow!("Failed to create shader"))?;

                gl.shader_source(shader, format!("#version 330\n{}", *source).as_str());
                gl.compile_shader(shader);

                if !gl.get_shader_compile_status(shader) {
                    bail!(gl.get_shader_info_log(shader));
                }

                gl.attach_shader(program, shader);
                *handle = Some(shader);
            }

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                bail!(gl.get_program_info_log(program));
            }

            // cleanup shaders
            for &(_, _, shader) in &shaders {
                gl.detach_shader(program, shader.unwrap());
                gl.delete_shader(shader.unwrap());
            }

            Ok(
                Self {
                    gl: gl.clone(),
                    program,
                    vertex_array
                }
            )
        }
    }

    pub fn draw(&mut self) -> Result<()> {
        unsafe {
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            self.gl.enable(glow::BLEND);
            self.gl.enable(glow::DEPTH_TEST);
            self.gl.enable(glow::ALPHA);
            
            self.gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

            self.gl.use_program(Some(self.program));
            self.gl.bind_vertex_array(Some(self.vertex_array));

            self.gl.draw_arrays(glow::TRIANGLES, 0, 3); // shaders are bound
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.vertex_array);
        }
    }
}