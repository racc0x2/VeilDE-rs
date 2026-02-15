use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    raw_window_handle::HasWindowHandle,
    window::Window,
    window::{WindowAttributes, WindowId}
};
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext as OpenGlContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface}
};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::{
    num::NonZeroU32,
    sync::mpsc::channel,
    sync::mpsc::Sender,
    time::Instant
};
use imgui_glow_renderer::{
    glow,
    AutoRenderer
};
use imgui::{
    internal::RawCast,
    FontConfig,
    FontSource,
};
use imgui_sys::{ImGuiFreeTypeBuilderFlags_Bitmap, ImGuiFreeType_GetBuilderForFreeType};
use anyhow::{anyhow, bail, Context, Error, Result};
use imgui::{Condition, Context as ImGuiContext};
use crate::renderer::VeilDERenderer;
use glutin::config::Config;
use imgui_glow_renderer::glow::HasContext;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::Event;
use winit::window::{Fullscreen, WindowLevel};
use winit::window::Fullscreen::Borderless;

const WINDOW_SIZE: [u32; 2] = [1600, 900];
const WINDOW_TITLE: &str = "VeilDE-rs";
const FONT_SIZE: f64 = 14.0;

#[allow(unused)] // contexts are all important, even if not currently used
struct VeilDEContexts {
    pub imgui: ImGuiContext,
    pub winit: WinitPlatform,
    pub window: Window,
    pub opengl: OpenGlContext,
    pub glow: AutoRenderer,
    pub surface: Surface<WindowSurface>,
}

struct VeilDEApplication {
    contexts: VeilDEContexts,
    renderer: VeilDERenderer,
    last_frame: Option<Instant>,
    resolution: PhysicalSize<u32>,
}

struct VeilDEApplicationHandler {
    application: Option<VeilDEApplication>,
    error_tx: Sender<Error>,
}

impl VeilDEApplicationHandler {
    pub fn new(error_tx: Sender<Error>) -> Self {
        Self {
            application: None,
            error_tx,
        }
    }
}

impl VeilDEApplication {
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        let (window, config) = init_glutin(event_loop)?;
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

        let contexts = VeilDEContexts {
            glow,
            imgui,
            opengl,
            winit,
            window,
            surface,
        };

        Ok(
            Self {
                renderer: VeilDERenderer::new(contexts.glow.gl_context()).context("Failed to create VeilDE renderer")?,
                contexts,
                last_frame: None,
                resolution: PhysicalSize::new(0, 0),
            }
        )
    }

    pub fn pre_window_event(&mut self, event: &WindowEvent) {
        self.contexts.winit.handle_event::<WindowEvent>(
            self.contexts.imgui.io_mut(),
            &self.contexts.window,
            &Event::WindowEvent {
                window_id: self.contexts.window.id(),
                event: event.clone()
            },
        );
    }

    pub fn post_window_event(&mut self, _: &WindowEvent) {
        self.contexts.window.request_redraw();
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.renderer.shutdown();

        Ok(())
    }

    fn gui(&mut self) -> Result<()> {
        let ui = self.contexts.imgui.new_frame();

        ui.window("VeilDE")
            .size([73f32, 73f32], Condition::FirstUseEver)
            .build(|| -> Result<()> {
                if ui.button("blow up") {
                    bail!("boom");
                }

                Ok(())
            }).unwrap_or(Ok(()))?;

        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        let now = Instant::now();
        self.contexts.imgui.io_mut().update_delta_time(now - self.last_frame.unwrap_or(now));
        self.last_frame = Some(now);

        // no safe way to achieve this
        unsafe { self.contexts.glow.gl_context().clear(glow::COLOR_BUFFER_BIT); }

        //self.renderer.draw().context("Failed to render VeilDE")?;
        self.gui().context("Failed to render VeilDE GUI")?;

        self.contexts.glow
            .render(self.contexts.imgui.render())
            .map_err(|_| anyhow!("Failed to render ImGui renderer data"))?;

        self.contexts.surface
            .swap_buffers(&self.contexts.opengl)
            .context("Failed to swap surface buffers")?;

        Ok(())
    }
}

impl ApplicationHandler for VeilDEApplicationHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.application.is_none() {
            match VeilDEApplication::new(event_loop) {
                Ok(app) => self.application = Some(app),
                Err(e) => {
                    // unavoidable crash ahead
                    self.error_tx.send(e).expect("Failed to send error");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let mut perform = || -> Result<()> {
            if let Some(app) = self.application.as_mut() {
                app.pre_window_event(&event);

                match event {
                    WindowEvent::CloseRequested => {
                        app.shutdown().context("Failed to shutdown VeilDE application")?;
                        event_loop.exit()
                    },

                    WindowEvent::RedrawRequested => {
                        app.render().context("Failed to draw VeilDE application")?;
                    }

                    _ => { }
                }

                app.post_window_event(&event);
            }

            Ok(())
        };

        if let Err(e) = perform() {
            // unavoidable crash ahead.
            self.error_tx.send(e).expect("Failed to send error");
            event_loop.exit();
        }
    }
}

fn get_font_data(scale: f64) -> Vec<FontSource<'static>> {
    vec![
        FontSource::TtfData {
            data: include_bytes!("../resources/segoeui.ttf"), // TODO: load dynamically
            size_pixels: (FONT_SIZE * scale) as f32,
            config: Some(FontConfig {
                rasterizer_multiply: 1f32,
                font_builder_flags: ImGuiFreeTypeBuilderFlags_Bitmap,

                oversample_h: 1i32,
                oversample_v: 1i32,
                glyph_offset: [0f32, (-5f64 * scale) as f32], // TODO: calculate dynamically by checking for blank pixels at the edge of the font atlas

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
    context.fonts().add_font(get_font_data(1f64).as_slice());

    Ok(context)
}

fn init_winit(imgui: &mut ImGuiContext, window: &Window) -> Result<WinitPlatform> {
    let mut context = WinitPlatform::new(imgui);
    context.attach_window(imgui.io_mut(), window, HiDpiMode::Default);

    Ok(context)
}

fn init_glutin(event_loop: &ActiveEventLoop) -> Result<(Window, Config)> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(
            || event_loop
                .available_monitors()
                .next()
        ).context("Failed to get monitor")?;

    let video_mode = monitor.video_modes().next().context("Failed to get video mode")?;

    let (window, config) = glutin_winit::DisplayBuilder::new()
        .with_window_attributes(Some(
            WindowAttributes::default()
                .with_title(WINDOW_TITLE)
                .with_transparent(true)
                .with_inner_size(video_mode.size())
                .with_fullscreen(Some(Borderless(None))) // TODO: fullscreen and transparent don't work together
                .with_transparent(true)
                .with_position(PhysicalPosition::new(0i32, 0i32))
                .with_decorations(true)
                //.with_window_level(WindowLevel::AlwaysOnBottom)
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

    // winit advises using Poll for vertically synced apps
    event_loop.set_control_flow(ControlFlow::Poll);

    let (tx, rx) = channel::<Error>();

    event_loop.run_app(
        &mut VeilDEApplicationHandler::new(tx)
    ).context("Failed to run app loop")?;

    if let Ok(error) = rx.try_recv() {
        bail!(error)
    }

    Ok(())
}