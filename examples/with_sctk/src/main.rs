use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_output, delegate_registry, delegate_seat, delegate_xdg_shell,
    delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::xdg::{
        window::{Window, WindowConfigure, WindowHandler, XdgWindowState},
        XdgShellState,
    },
};
use vello::{
    block_on_wgpu,
    kurbo::{Affine, Point, Rect, Vec2},
    peniko::{Brush, Color, Fill},
    util::{RenderContext, RenderSurface},
    Renderer, Scene, SceneBuilder,
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_seat, wl_surface},
    Connection, EventQueue, Proxy, QueueHandle,
};

#[derive(Clone)]
struct WindowHandle {
    conn: Connection,
    window: Window,
}

struct AppState {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    xdg_window_state: XdgWindowState,
    surface: RenderSurface,

    handle: WindowHandle,
    render_cx: RenderContext,
    renderer: Renderer,
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = WaylandWindowHandle::empty();
        handle.surface = self.window.wl_surface().id().as_ptr() as *mut _;
        RawWindowHandle::Wayland(handle)
    }
}

unsafe impl HasRawDisplayHandle for WindowHandle {
    fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        let mut handle = WaylandDisplayHandle::empty();
        handle.display = self.conn.backend().display_ptr() as *mut _;
        RawDisplayHandle::Wayland(handle)
    }
}

fn main() {
    pretty_env_logger::init();
    pollster::block_on(run());
}

async fn run() {
    let conn = Connection::connect_to_env().unwrap();
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh: QueueHandle<AppState> = event_queue.handle();
    let compositor_state =
        CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
    let xdg_shell_state = XdgShellState::bind(&globals, &qh).expect("xdg shell not available");
    let mut xdg_window_state = XdgWindowState::bind(&globals, &qh);

    let surface = compositor_state.create_surface(&qh);
    // Create the window for adapter selection
    let window = Window::builder()
        .title("wgpu wayland window")
        // GitHub does not let projects use the `org.github` domain but the `io.github` domain is fine.
        .app_id("io.github.smithay.client-toolkit.WgpuExample")
        .min_size((256, 256))
        .map(&qh, &xdg_shell_state, &mut xdg_window_state, surface)
        .expect("window creation");

    let handle = WindowHandle { conn, window };

    let mut render_cx = RenderContext::new().unwrap();
    let surface = render_cx.create_surface(&handle, 256, 256).await;
    let device_handle = &render_cx.devices[surface.dev_id];
    let renderer = Renderer::new(&device_handle.device).unwrap();

    let mut state = AppState {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor_state,
        xdg_shell_state,
        xdg_window_state,
        handle,
        surface,
        render_cx,
        renderer,
    };

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }
}

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl WindowHandler for AppState {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        todo!()
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let width = self.surface.config.width;
        let height = self.surface.config.height;
        dbg!(width, height);
        let mut scene = Scene::new();
        let mut builder = SceneBuilder::for_scene(&mut scene);
        let rect = Rect::from_origin_size(Point::new(0.0, 0.0), (1000.0, 1000.0));
        builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(Color::rgb8(128, 128, 128)),
            None,
            &rect,
        );
        builder.finish();
        let surface_texture = self
            .surface
            .surface
            .get_current_texture()
            .expect("failed to get surface texture");
        let device_handle = &self.render_cx.devices[self.surface.dev_id];
        block_on_wgpu(
            &device_handle.device,
            self.renderer.render_to_surface_async(
                &device_handle.device,
                &device_handle.queue,
                &scene,
                &surface_texture,
                width,
                height,
            ),
        )
        .expect("failed to render to surface");
        surface_texture.present();
        //device_handle.device.poll(wgpu::Maintain::Poll);
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _capability: Capability,
    ) {
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

delegate_compositor!(AppState);
delegate_output!(AppState);

delegate_seat!(AppState);

delegate_xdg_shell!(AppState);
delegate_xdg_window!(AppState);

delegate_registry!(AppState);

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}
