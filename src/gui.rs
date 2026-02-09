use crate::utils::{winit_button_to_imgui_button, winit_key_to_imgui_key};
use std::num::NonZeroU32;
use std::process::exit;
use anyhow::{Context, Result};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use imgui_sys::*;
use imgui_glow_renderer::{
    glow,
    AutoRenderer,
    glow::HasContext
};
use imgui::{
    FontSource,
    FontConfig,
    Ui,
    internal::RawCast
};
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface}
};
use winit::{
    window::Window,
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{WindowAttributes, WindowId},
    dpi::{LogicalSize, Size},
    raw_window_handle::HasWindowHandle
};

pub struct GuiContexts {
    pub imgui: imgui::Context,
    pub platform: WinitPlatform,
    pub window: Window,
    pub opengl: PossiblyCurrentContext,
    pub glow: AutoRenderer,
    pub surface: Surface<WindowSurface>,
}

struct ImGuiApplication {
    contexts: GuiContexts,
}

impl ImGuiApplication {
    pub fn new(contexts: GuiContexts) -> Self {
        Self { contexts }
    }
    pub fn draw(ui: &mut Ui) {
        ui.show_demo_window(&mut true);
    }
}

impl ApplicationHandler for ImGuiApplication {
    fn resumed(&mut self, _: &ActiveEventLoop) {
        //unimplemented!()
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                exit(0);
            }

            WindowEvent::RedrawRequested => {
                let contexts = &mut self.contexts;

                unsafe { contexts.glow.gl_context().clear(glow::COLOR_BUFFER_BIT) };
                let ui = contexts.imgui.new_frame();

                Self::draw(ui);

                contexts.platform
                    .prepare_render(ui, &contexts.window);

                contexts.glow
                    .render(contexts.imgui.render())
                    .expect("Failed to render ImGui data");

                contexts.surface
                    .swap_buffers(&contexts.opengl)
                    .expect("Failed to swap surface buffers");
            }

            WindowEvent::KeyboardInput {
                event, ..
            } => {
                if let Some(key) = winit_key_to_imgui_key(event.physical_key) {
                    self.contexts.imgui
                        .io_mut()
                        .add_key_event(
                            key,
                            event.state == ElementState::Pressed
                        );
                }
            }

            WindowEvent::CursorMoved {
                position, ..
            } => {
                self.contexts.imgui.io_mut().add_mouse_pos_event([position.x as f32, position.y as f32]);
            },

            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(x, y), ..
            } => self.contexts.imgui.io_mut().add_mouse_wheel_event([x, y]),

            WindowEvent::MouseInput {
                 button, state, ..
            } => {
                let io = self.contexts.imgui.io_mut();

                if let Some(b) = winit_button_to_imgui_button(button) {
                    io.add_mouse_button_event(
                            b, state.is_pressed()
                        );
                }
            }

            _ => { }
        }

        self.contexts.window.request_redraw();
    }
}
pub fn init() -> Result<()> {
    let mut imgui = imgui::Context::create();

    unsafe {
        imgui.fonts().raw_mut().FontBuilderIO = ImGuiFreeType_GetBuilderForFreeType();
    }

    imgui.io_mut().font_global_scale = 1f32;

    imgui.fonts().add_font(&[
        FontSource::TtfData {
            data: include_bytes!("../resources/segoeui.ttf"),
            size_pixels: 14.0f32,
            config: Some(FontConfig {
                rasterizer_multiply: 1.0,
                font_builder_flags: ImGuiFreeTypeBuilderFlags_Bitmap,

                oversample_h: 1,
                oversample_v: 1,
                glyph_offset: [0f32, -5f32], // TODO: calculate dynamically by checking for blank pixels at the edge of the font atlas

                ..FontConfig::default()
            })
        },

        /*
        FontSource::DefaultFontData {
            config: None
        }
        */
    ]);

    imgui.set_ini_filename(None);

    let mut platform = WinitPlatform::new(&mut imgui);
    let event_loop = EventLoop::new()
        .context("Failed to create event loop")?;

    let (window, config) = glutin_winit::DisplayBuilder::new()
        .with_window_attributes(Some(
            WindowAttributes::default()
                .with_title("VeilDE-rs")
                .with_transparent(true)
                .with_fullscreen(None)
                .with_inner_size(Size::Logical(LogicalSize::new(1600f64, 900f64)))
            )
        ).build(
            &event_loop,
            ConfigTemplateBuilder::new(),
            |mut cfg| {
                cfg.next().context("Failed to get next configuration value").unwrap()
            }
        ).map_err(|_| anyhow::anyhow!("Failed to create window"))?;

    let window = window.context("Failed to retrieve window")?;

    let opengl = unsafe {
        config.display().create_context(
            &config,
            &ContextAttributesBuilder::new()
                .build(
                    Some(
                        window
                            .window_handle()
                            .context("Failed to get window handle for context")?
                            .as_raw()
                    )
                )
        ).expect("Failed to create OpenGL context")
    };

    let surface = unsafe {
        config
            .display()
            .create_window_surface(
                &config,
                &SurfaceAttributesBuilder::<WindowSurface>::new()
                    .with_srgb(Some(true))
                    .build(
                        window
                            .window_handle()
                            .context("Failed to get window handle for surface")?
                            .as_raw(),
                        NonZeroU32::new(1600u32).expect("Window surface width was zero or out-of-bounds"),
                        NonZeroU32::new(600u32).expect("Window surface height was zero or out-of-bounds"),
                    )
            )
            .expect("Failed to create window surface")
    };

    let opengl = opengl
        .make_current(&surface)
        .expect("Failed to make OpenGL context current");

    surface.set_swap_interval(
        &opengl,
        SwapInterval::Wait(
            NonZeroU32::new(1)
                .expect("Swap interval was zero or out-of-bounds")
        )
    ).expect("Failed to set swap interval");

    let glow = AutoRenderer::new(
        unsafe {
            glow::Context::from_loader_function_cstr(
                |s| {
                    opengl
                        .display()
                        .get_proc_address(s)
                        .cast()
                })
        }, &mut imgui).expect("Failed to create GLOW context");

    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Rounded);

    let contexts = GuiContexts {
        glow,
        surface,
        opengl,

        imgui,
        platform,
        window,
    };

    event_loop.set_control_flow(ControlFlow::Poll); // uncapped, be careful that the graphics api is set to vsync or capped lol

    let mut app = ImGuiApplication::new(contexts);
    event_loop.run_app(&mut app).context("Failed to run app loop")?;

    Ok(())
}