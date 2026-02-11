use crate::utils::{winit_button_to_imgui_button, winit_key_to_imgui_key};
use anyhow::{anyhow, bail, Context, Error, Result};
use glutin::config::Config;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext as OpenGlContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface}
};
use imgui::{Condition, Context as ImGuiContext};
use imgui::{
    internal::RawCast,
    FontConfig,
    FontSource,
};
use imgui_glow_renderer::{
    glow,
    glow::HasContext,
    AutoRenderer
};
use imgui_sys::{ImGuiFreeType_GetBuilderForFreeType, ImGuiFreeTypeBuilderFlags_Bitmap};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, Size},
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    raw_window_handle::HasWindowHandle,
    window::Window,
    window::{WindowAttributes, WindowId}
};
use std::{
    sync::mpsc::channel,
    sync::mpsc::Sender,
    time::Instant,
    num::NonZeroU32
};

#[allow(unused)] // contexts are all important, even if not currently used
pub struct GuiContexts {
    pub imgui: ImGuiContext,
    pub winit: WinitPlatform,
    pub window: Window,
    pub opengl: OpenGlContext,
    pub glow: AutoRenderer,
    pub surface: Surface<WindowSurface>,
}

const WINDOW_SIZE: [u32; 2] = [1600, 900];
const WINDOW_TITLE: &str = "VeilDE-rs";
const FONT_SIZE: f32 = 14.0f32;

struct VeilDEApplication {
    contexts: GuiContexts,
    last_frame: Option<Instant>,
    error_tx: Sender<Error>,
}

impl VeilDEApplication {
    pub fn new(contexts: GuiContexts, error_tx: Sender<Error>) -> Self {
        Self { contexts, last_frame: None, error_tx }
    }

    pub fn draw(&mut self) -> Result<()> {
        let ui = self.contexts.imgui.new_frame();
        {
            ui.window("VeilDE")
                .size([300.0, 100.0], Condition::FirstUseEver)
                .build(|| -> Result<()> {
                    if ui.button("blow up") {
                        bail!("boom");
                    }

                    Ok(())
                }).unwrap_or(Ok(()))?;
        }

        Ok(())
    }
}

impl ApplicationHandler for VeilDEApplication {
    fn resumed(&mut self, _: &ActiveEventLoop) {
        // for mobile platforms. this is a Windows exclusive app
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let perform = || -> Result<()> {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),

                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    self.contexts.imgui.io_mut().update_delta_time(now - self.last_frame.unwrap_or(now));
                    self.last_frame = Some(now);

                    // clearing the color bit can fail,
                    // and glow does not provide a way
                    // to detect or recover from that error
                    unsafe { self.contexts.glow.gl_context().clear(glow::COLOR_BUFFER_BIT) };

                    self.draw()?;

                    self.contexts.glow
                        .render(self.contexts.imgui.render())
                        .map_err(|_| anyhow!("Failed to render ImGui renderer data"))?;

                    self.contexts.surface
                        .swap_buffers(&self.contexts.opengl)
                        .context("Failed to swap surface buffers")?;
                }

                WindowEvent::Resized(size) => {
                    if size.width > 0 && size.height > 0 {
                        self.contexts.surface.resize(
                            &self.contexts.opengl,
                            NonZeroU32::new(size.width).unwrap(),
                            NonZeroU32::new(size.height).unwrap()
                        );

                        // must interface with the gl context
                        // unsafe operation, unavoidable
                        unsafe {
                            self.contexts.glow.gl_context().viewport(
                                0, 0,
                                size.width as i32, size.height as i32
                            );
                        }
                    }
                }

                WindowEvent::KeyboardInput { event, .. } => {
                    if let Some(key) = winit_key_to_imgui_key(event.physical_key) {
                        self.contexts.imgui
                            .io_mut()
                            .add_key_event(
                                key,
                                event.state == ElementState::Pressed
                            );
                    }
                },

                WindowEvent::CursorMoved { position, .. } =>
                    self.contexts.imgui.io_mut().add_mouse_pos_event([position.x as f32, position.y as f32]),

                WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(x, y), .. } =>
                    self.contexts.imgui.io_mut().add_mouse_wheel_event([x, y]),

                WindowEvent::MouseInput { button, state, .. } => {
                    if let Some(b) = winit_button_to_imgui_button(button) {
                        self.contexts.imgui.io_mut().add_mouse_button_event(b, state.is_pressed());
                    }
                }

                _ => { }
            }

            self.contexts.window.request_redraw();
            Ok(())
        };

        if let Err(e) = perform() {
            // unavoidable crash ahead.
            self.error_tx.send(e).expect("Failed to send error");
            event_loop.exit();
        }
    }
}

fn get_font_data(scale: f32) -> Vec<FontSource<'static>> {
    vec![
        FontSource::TtfData {
            data: include_bytes!("../resources/segoeui.ttf"), // TODO: load dynamically
            size_pixels: FONT_SIZE * scale,
            config: Some(FontConfig {
                rasterizer_multiply: 1f32,
                font_builder_flags: ImGuiFreeTypeBuilderFlags_Bitmap,

                oversample_h: 1i32,
                oversample_v: 1i32,
                glyph_offset: [0f32, -5f32 * scale], // TODO: calculate dynamically by checking for blank pixels at the edge of the font atlas

                ..FontConfig::default()
            })
        },
    ]
}

fn init_imgui() -> Result<ImGuiContext> {
    let mut context = ImGuiContext::create();

    context.set_ini_filename(None);

    // freetype doesn't enable itself
    // due to a bug in the 'imgui-sys'
    // crate, that has yet to be patched
    //
    // https://github.com/imgui-rs/imgui-rs/issues/773
    unsafe { context.fonts().raw_mut().FontBuilderIO = ImGuiFreeType_GetBuilderForFreeType(); }
    context.io_mut().font_global_scale = 1f32; // scale through font data for high quality
    context.fonts().add_font(get_font_data(1f32).as_slice());

    Ok(context)
}

fn init_winit(imgui: &mut ImGuiContext, window: &Window) -> Result<WinitPlatform> {
    let mut context = WinitPlatform::new(imgui);
    context.attach_window(imgui.io_mut(), window, HiDpiMode::Rounded);

    Ok(context)
}

fn init_glutin(event_loop: &EventLoop<()>) -> Result<(Window, Config)> {
    let (window, config) = glutin_winit::DisplayBuilder::new()
        .with_window_attributes(Some(
            WindowAttributes::default()
                .with_title(WINDOW_TITLE)
                .with_transparent(true)
                .with_fullscreen(None)
                .with_inner_size(
                    Size::Logical(
                        LogicalSize::new(
                            WINDOW_SIZE[0] as f64,
                            WINDOW_SIZE[1] as f64
                        )
                    )
                )
        )
        ).build(
        event_loop,
        ConfigTemplateBuilder::new(),
        |mut cfg| {
            cfg.next().context("Failed to get next configuration value").unwrap()
        }
    ).map_err(|_| anyhow!("Failed to initialize glutin"))?;

    Ok(
        (window.context("Failed to create window")?, config)
    )
}

fn init_opengl(window: &Window, config: &Config) -> Result<(OpenGlContext, Surface<WindowSurface>)> {
    // glutin does not provide a
    // safe alternative to creating
    // display contexts with winit
    let context = unsafe {
        config.display().create_context(
            config,
            &ContextAttributesBuilder::new()
                .build(
                    Some(
                        window
                            .window_handle()
                            .context("Failed to get window handle for context")?
                            .as_raw()
                    )
                )
        ).context("Failed to create OpenGL context")?
    };

    // glutin does not provide a safe
    // alternative to creating window
    // surfaces with winit
    let surface = unsafe {
        config
            .display()
            .create_window_surface(
                config,
                &SurfaceAttributesBuilder::<WindowSurface>::new()
                    .with_srgb(Some(true))
                    .build(
                        window
                            .window_handle()
                            .context("Failed to get window handle for surface")?
                            .as_raw(),
                        NonZeroU32::new(WINDOW_SIZE[0]).context("Window surface width was zero or out-of-bounds")?,
                        NonZeroU32::new(WINDOW_SIZE[1]).context("Window surface height was zero or out-of-bounds")?,
                    )
            )
            .context("Failed to create window surface")?
    };

    Ok((
        context.make_current(&surface)
            .context("Failed to make OpenGL context current")?,

        surface
    ))
}

fn init_glow(opengl: &OpenGlContext, imgui: &mut ImGuiContext) -> Result<AutoRenderer> {
    // glow requires using `get_proc_address`,
    // which is an inherently unsafe concept
    AutoRenderer::new(
        unsafe {
            glow::Context::from_loader_function_cstr(
                |s| {
                    opengl
                        .display()
                        .get_proc_address(s)
                        .cast()
                })
        }, imgui
    ).context("Failed to create GLOW context")
}

pub fn init() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let (window, config) = init_glutin(&event_loop)?;
    let (opengl, surface) = init_opengl(&window, &config)?;
    let mut imgui = init_imgui()?;
    let glow = init_glow(&opengl, &mut imgui)?;
    let winit = init_winit(&mut imgui, &window)?;

    surface.set_swap_interval(
        &opengl,
        SwapInterval::Wait(
            NonZeroU32::new(1)
                .context("Swap interval was zero or out-of-bounds")?
        )
    ).context("Failed to set swap interval")?;

    // winit advises using Poll for vertically synced apps
    event_loop.set_control_flow(ControlFlow::Poll);

    let (tx, rx) = channel::<Error>();

    event_loop.run_app(
        &mut VeilDEApplication::new(
            GuiContexts {
                glow,
                surface,
                opengl,
                imgui,
                winit,
                window,
            },
            tx
        )
    ).context("Failed to run app loop")?;

    if let Ok(error) = rx.try_recv() {
        bail!(error)
    }

    Ok(())
}