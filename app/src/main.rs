#![windows_subsystem = "windows"]

use anyhow::{anyhow, Context, Result};
use pty::{Pty, PtyReader, PtySize, PtyWriter};
use render::{
    CursorPosition, FontSpec, RenderError, RenderGrid, RenderSize, Renderer, CELL_HEIGHT,
    CELL_WIDTH, DEFAULT_FONT_SIZE, PADDING_X, PADDING_Y,
};
use screen::{Screen, ScreenSize};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::io::Cursor;
use std::thread;
use std::time::{Duration, Instant};
use std::{env, fs};
use tracing::{error, info, warn};
use vt::VtParser;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
#[cfg(windows)]
use winit::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use winit::window::WindowBuilder;
#[cfg(windows)]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const CASCADIA_DOWNLOAD_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/BENZOOgataga/RING0/main/install/Cascadia_Code.zip",
    "https://github.com/BENZOOgataga/RING0/raw/main/install/Cascadia_Code.zip",
];
const CASCADIA_ZIP_PATH: &str = "static/CascadiaCode-Regular.ttf";
const DEFAULT_SHELL_COMMAND: &str =
    "powershell.exe -NoLogo -NoProfile -NoExit -Command \"Remove-Module PSReadLine -ErrorAction SilentlyContinue\"";

struct AppState {
    window: winit::window::Window,
    renderer: Renderer<'static>,
    pty: Option<Pty>,
    pty_writer: Option<PtyWriter>,
    pty_rx: Option<Receiver<PtyMessage>>,
    vt_parser: VtParser,
    screen: Screen,
    render_cells: Vec<char>,
    pty_closed: bool,
    last_status_check: Instant,
    exit_checks_failed: u8,
    cursor_visible: bool,
    last_cursor_toggle: Instant,
    font_prompt: bool,
    font_download_rx: Option<Receiver<FontDownloadMessage>>,
    font_download_in_progress: bool,
    modifiers: ModifiersState,
    input_len: usize,
    input_buffer: String,
    exit_requested: bool,
}

enum PtyMessage {
    Data(Vec<u8>),
    Closed,
}

enum FontDownloadMessage {
    Completed(Result<Vec<u8>, String>),
}

impl AppState {
    async fn new(window: winit::window::Window) -> Result<Self> {
        let size = window.inner_size();
        let render_size = RenderSize {
            width: size.width.max(1),
            height: size.height.max(1),
        };
        let screen_size = screen_size_from_pixels(size);

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(&window)
            .context("create wgpu surface")?;
        let surface = unsafe {
            // Surface lifetime is tied to the window; AppState owns the window for program life.
            std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface)
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow!("no suitable GPU adapter found"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("request wgpu device")?;

        let font_load = load_font_bytes().context("load font data")?;
        info!("font source: {:?}", font_load.source);
        let renderer = Renderer::new(
            surface,
            &adapter,
            device,
            queue,
            render_size,
            FontSpec {
                bytes: font_load.bytes,
                size: DEFAULT_FONT_SIZE,
            },
        )
        .context("initialize renderer")?;

        let screen = Screen::new(screen_size).context("initialize screen")?;
        let mut state = Self {
            window,
            renderer,
            pty: None,
            pty_writer: None,
            pty_rx: None,
            vt_parser: VtParser::new(),
            screen,
            render_cells: Vec::new(),
            pty_closed: false,
            last_status_check: Instant::now(),
            exit_checks_failed: 0,
            cursor_visible: true,
            last_cursor_toggle: Instant::now(),
            font_prompt: font_load.source == FontSource::Fallback,
            font_download_rx: None,
            font_download_in_progress: false,
            modifiers: ModifiersState::default(),
            input_len: 0,
            input_buffer: String::new(),
            exit_requested: false,
        };

        if state.font_prompt {
            state.show_font_prompt();
        } else {
            state.start_pty()?;
        }

        Ok(state)
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        if let Err(err) = self.renderer.resize(RenderSize {
            width: new_size.width,
            height: new_size.height,
        }) {
            warn!("renderer resize failed: {err}");
        }

        let screen_size = screen_size_from_pixels(new_size);
        if screen_size != self.screen.size() {
            if let Err(err) = self.screen.resize(screen_size) {
                warn!("screen resize failed: {err}");
            }
            if let Some(pty) = self.pty.as_mut() {
                if let Err(err) = pty.resize(PtySize {
                    cols: screen_size.cols,
                    rows: screen_size.rows,
                }) {
                    warn!("pty resize failed: {err}");
                }
            }
        }
    }

    fn handle_input_text(&mut self, text: &str) {
        if self.pty_closed {
            return;
        }
        if self.font_prompt {
            self.handle_font_prompt_input(text);
            return;
        }
        let mut filtered = String::new();
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            filtered.push(ch);
        }
        if !filtered.is_empty() {
            self.input_len = self.input_len.saturating_add(filtered.chars().count());
            self.input_buffer.push_str(&filtered);
            self.send_input_bytes(filtered.as_bytes());
        }
    }

    fn handle_special_key(&mut self, key: NamedKey) {
        if self.pty_closed {
            return;
        }
        if self.font_prompt {
            return;
        }
        let bytes: Option<&[u8]> = match key {
            NamedKey::Enter => {
                if self.input_buffer.trim().eq_ignore_ascii_case("exit") {
                    self.exit_requested = true;
                    self.pty_closed = true;
                }
                self.input_len = 0;
                self.input_buffer.clear();
                Some(b"\r".as_slice())
            }
            NamedKey::Backspace => {
                if self.input_len > 0 {
                    self.input_len -= 1;
                    self.input_buffer.pop();
                    Some(&[0x08u8] as &[u8])
                } else {
                    None
                }
            }
            NamedKey::Escape => Some(&[0x1Bu8] as &[u8]),
            NamedKey::Tab => Some(&[0x09u8] as &[u8]),
            _ => None,
        };

        if let Some(bytes) = bytes {
            self.send_input_bytes(bytes);
        }
    }

    fn drain_pty(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = self.pty_rx.as_ref() {
            while let Ok(message) = rx.try_recv() {
                match message {
                    PtyMessage::Data(bytes) => {
                        self.vt_parser.advance(&bytes, &mut events);
                        if !events.is_empty() {
                            self.screen.apply_events(&events);
                            events.clear();
                        }
                    }
                    PtyMessage::Closed => {
                        self.pty_closed = true;
                        self.exit_checks_failed = 0;
                        info!("pty closed; stopping input");
                    }
                }
            }
        }
    }

    fn check_pty_status(&mut self) {
        if self.pty_closed {
            return;
        }
        let Some(pty) = self.pty.as_ref() else {
            return;
        };
        if self.last_status_check.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_status_check = Instant::now();
        match pty.is_running() {
            Ok(true) => {
                self.exit_checks_failed = 0;
            }
            Ok(false) => {
                self.exit_checks_failed = self.exit_checks_failed.saturating_add(1);
                if self.exit_checks_failed >= 2 {
                    self.pty_closed = true;
                    info!("pty no longer running; exiting");
                }
            }
            Err(err) => {
                warn!("pty status check failed: {err}");
            }
        }
    }

    fn drain_font_download(&mut self) {
        let mut message = None;
        if let Some(rx) = self.font_download_rx.as_ref() {
            while let Ok(next) = rx.try_recv() {
                message = Some(next);
            }
        }

        let Some(message) = message else {
            return;
        };

        self.font_download_rx = None;
        self.font_download_in_progress = false;

        match message {
            FontDownloadMessage::Completed(Ok(bytes)) => {
                if let Err(err) = self.apply_downloaded_font(bytes) {
                    warn!("font download apply failed: {err}");
                    self.show_font_download_error(&format!(
                        "Failed to apply downloaded font: {err}"
                    ));
                    return;
                }
                self.font_prompt = false;
                if let Err(err) = self.start_pty() {
                    warn!("pty start failed: {err}");
                    self.show_system_message(&format!(
                        "Failed to start shell: {err}\r\nClose the window to exit.\r\n"
                    ));
                }
            }
            FontDownloadMessage::Completed(Err(err)) => {
                self.show_font_download_error(&err);
            }
        }
    }

    fn handle_font_prompt_input(&mut self, text: &str) {
        if self.font_download_in_progress {
            return;
        }
        let mut choice = None;
        for ch in text.chars() {
            match ch {
                'y' | 'Y' => {
                    choice = Some(true);
                    break;
                }
                'n' | 'N' => {
                    choice = Some(false);
                    break;
                }
                _ => {}
            }
        }

        match choice {
            Some(true) => self.begin_font_download(),
            Some(false) => {
                self.font_prompt = false;
                if let Err(err) = self.start_pty() {
                    warn!("pty start failed: {err}");
                    self.show_system_message(&format!(
                        "Failed to start shell: {err}\r\nClose the window to exit.\r\n"
                    ));
                }
            }
            None => {}
        }
    }

    fn begin_font_download(&mut self) {
        if self.font_download_in_progress {
            return;
        }
        self.font_download_in_progress = true;
        self.show_font_download_pending();
        self.font_download_rx = Some(spawn_font_download());
    }

    fn show_system_message(&mut self, text: &str) {
        self.screen.clear();
        self.screen.scroll_to_bottom();
        let mut events = Vec::new();
        self.vt_parser.advance(text.as_bytes(), &mut events);
        self.screen.apply_events(&events);
    }

    fn show_font_prompt(&mut self) {
        self.show_system_message(
            "Cascadia Code not found.\r\n\
Press Y to download it (uses network) or N to continue with the fallback font.\r\n",
        );
    }

    fn show_font_download_pending(&mut self) {
        self.show_system_message("Downloading Cascadia Code...\r\n");
    }

    fn show_font_download_error(&mut self, err: &str) {
        self.font_prompt = true;
        self.show_system_message(&format!(
            "Download failed: {err}\r\n\
Press Y to retry or N to continue with the fallback font.\r\n"
        ));
    }

    fn apply_downloaded_font(&mut self, bytes: Vec<u8>) -> Result<()> {
        self.renderer
            .set_font(FontSpec {
                bytes: bytes.clone(),
                size: DEFAULT_FONT_SIZE,
            })
            .context("update renderer font")?;
        info!("font source: {:?}", FontSource::Cascadia);
        if let Some(path) = font_cache_path()? {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context("create font cache dir")?;
            }
            fs::write(&path, &bytes).context("write font cache")?;
        }
        Ok(())
    }

    fn start_pty(&mut self) -> Result<()> {
        let size = self.screen.size();
        let pty = Pty::spawn(
            DEFAULT_SHELL_COMMAND,
            PtySize {
                cols: size.cols,
                rows: size.rows,
            },
        )
        .context("spawn pty")?;
        let reader = pty.reader().context("clone pty reader")?;
        let writer = pty.writer().context("clone pty writer")?;
        let rx = spawn_pty_reader(reader);

        self.font_prompt = false;
        self.pty = Some(pty);
        self.pty_writer = Some(writer);
        self.pty_rx = Some(rx);
        self.pty_closed = false;
        self.last_status_check = Instant::now();
        self.exit_checks_failed = 0;
        self.input_len = 0;
        self.input_buffer.clear();
        self.exit_requested = false;
        self.screen.clear();
        self.screen.scroll_to_bottom();
        Ok(())
    }

    fn send_input_bytes(&mut self, bytes: &[u8]) {
        self.screen.scroll_to_bottom();
        if let Some(writer) = self.pty_writer.as_mut() {
            if let Err(err) = writer.write_all(bytes) {
                warn!("pty write failed: {err}");
            }
        }
    }

    fn render(&mut self) {
        self.drain_pty();
        if self.pty_closed {
            return;
        }

        self.screen.render_chars(&mut self.render_cells);

        let cursor = if self.pty_closed || self.screen.is_scrolled() {
            None
        } else {
            let cursor = self.screen.cursor();
            Some(CursorPosition {
                col: cursor.col,
                row: cursor.row,
            })
        };

        let grid = RenderGrid {
            cols: self.screen.size().cols,
            rows: self.screen.size().rows,
            cells: &self.render_cells,
            cursor,
            cursor_visible: self.cursor_visible,
        };

        match self.renderer.render(&grid) {
            Ok(()) => {}
            Err(RenderError::Surface(wgpu::SurfaceError::Lost)) => {
                if let Err(err) = self.renderer.resize(self.renderer_size()) {
                    warn!("surface lost; resize failed: {err}");
                }
            }
            Err(RenderError::Surface(wgpu::SurfaceError::Outdated)) => {
                if let Err(err) = self.renderer.resize(self.renderer_size()) {
                    warn!("surface outdated; resize failed: {err}");
                }
            }
            Err(RenderError::Surface(wgpu::SurfaceError::Timeout)) => {
                warn!("surface timeout during render");
            }
            Err(err) => {
                error!("render error: {err}");
            }
        }
    }

    fn renderer_size(&self) -> RenderSize {
        RenderSize {
            width: self.window.inner_size().width.max(1),
            height: self.window.inner_size().height.max(1),
        }
    }

    fn update_cursor_blink(&mut self) {
        if self.pty_closed {
            self.cursor_visible = false;
            return;
        }
        if self.last_cursor_toggle.elapsed() >= Duration::from_millis(600) {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_toggle = Instant::now();
        }
    }
}

fn spawn_pty_reader(reader: PtyReader) -> Receiver<PtyMessage> {
    let (tx, rx) = mpsc::channel();
    spawn_reader_thread(tx, reader);
    rx
}

fn spawn_reader_thread(tx: Sender<PtyMessage>, mut reader: PtyReader) {
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = tx.send(PtyMessage::Closed);
                    break;
                }
                Ok(n) => {
                    if tx.send(PtyMessage::Data(buffer[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    warn!("pty read failed: {err}");
                    let _ = tx.send(PtyMessage::Closed);
                    break;
                }
            }
        }
    });
}

fn screen_size_from_pixels(size: winit::dpi::PhysicalSize<u32>) -> ScreenSize {
    let usable_width = size.width.saturating_sub(PADDING_X * 2);
    let usable_height = size.height.saturating_sub(PADDING_Y * 2);
    let cols = (usable_width / CELL_WIDTH).max(1) as u16;
    let rows = (usable_height / CELL_HEIGHT).max(1) as u16;
    ScreenSize { cols, rows }
}

fn control_code_for_char(ch: char) -> Option<u8> {
    let ch = ch.to_ascii_uppercase();
    if ('A'..='Z').contains(&ch) {
        Some((ch as u8) - b'A' + 1)
    } else {
        None
    }
}

fn spawn_font_download() -> Receiver<FontDownloadMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = download_cascadia_font();
        let _ = tx.send(FontDownloadMessage::Completed(result));
    });
    rx
}

fn download_cascadia_font() -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("RING0/0.1")
        .build()
        .map_err(|err| err.to_string())?;
    let mut last_error = None;
    for url in CASCADIA_DOWNLOAD_URLS {
        let response = match client.get(*url).send() {
            Ok(response) => response,
            Err(err) => {
                last_error = Some(err.to_string());
                continue;
            }
        };
        if !response.status().is_success() {
            last_error = Some(format!("HTTP {} from {url}", response.status()));
            continue;
        }
        let bytes = response.bytes().map_err(|err| err.to_string())?;
        return extract_cascadia_from_zip(bytes.to_vec());
    }
    Err(last_error.unwrap_or_else(|| "download failed".to_string()))
}

fn extract_cascadia_from_zip(zip_bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|err| err.to_string())?;
    let mut file = archive
        .by_name(CASCADIA_ZIP_PATH)
        .map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    use std::io::Read;
    file.read_to_end(&mut out).map_err(|err| err.to_string())?;
    Ok(out)
}

fn load_font_bytes() -> Result<FontLoad> {
    if let Some(path) = font_cache_path()? {
        if let Ok(bytes) = fs::read(&path) {
            return Ok(FontLoad {
                bytes,
                source: FontSource::Cascadia,
            });
        }
    }

    let cascadia = [
        r"C:\Windows\Fonts\CascadiaCode.ttf",
        r"C:\Windows\Fonts\CascadiaCodePL.ttf",
    ];
    for path in cascadia {
        if let Ok(bytes) = fs::read(path) {
            return Ok(FontLoad {
                bytes,
                source: FontSource::Cascadia,
            });
        }
    }

    let fallback = [
        r"C:\Windows\Fonts\consola.ttf",
        r"C:\Windows\Fonts\lucon.ttf",
    ];
    for path in fallback {
        if let Ok(bytes) = fs::read(path) {
            return Ok(FontLoad {
                bytes,
                source: FontSource::Fallback,
            });
        }
    }

    Err(anyhow!(
        "no supported font found in Windows Fonts (expected Cascadia Code or Consolas)"
    ))
}

struct FontLoad {
    bytes: Vec<u8>,
    source: FontSource,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FontSource {
    Cascadia,
    Fallback,
}

fn font_cache_path() -> Result<Option<PathBuf>> {
    let base = env::var("LOCALAPPDATA").ok();
    let base = match base {
        Some(base) => PathBuf::from(base),
        None => return Ok(None),
    };
    Ok(Some(
        base.join("RING0").join("fonts").join("CascadiaCode.ttf"),
    ))
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let event_loop = EventLoop::new().context("create event loop")?;
    let default_width = CELL_WIDTH * 120 + PADDING_X * 2;
    let default_height = CELL_HEIGHT * 30 + PADDING_Y * 2;
    let mut window_builder = WindowBuilder::new()
        .with_title("RING0")
        .with_inner_size(winit::dpi::PhysicalSize::new(default_width, default_height));
    let icon = build_terminal_icon();
    if let Some(icon) = icon.as_ref() {
        window_builder = window_builder.with_window_icon(Some(icon.clone()));
        #[cfg(windows)]
        {
            window_builder = window_builder.with_taskbar_icon(Some(icon.clone()));
        }
    }
    let window = window_builder
        .build(&event_loop)
        .context("create window")?;
    if let Some(icon) = icon {
        window.set_window_icon(Some(icon.clone()));
        #[cfg(windows)]
        window.set_taskbar_icon(Some(icon));
    }
    #[cfg(windows)]
    apply_taskbar_icon_resource(&window);

    let mut state = pollster::block_on(AppState::new(window))?;

    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Wait);
        match event {
            Event::WindowEvent { event, window_id } if window_id == state.window.id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        target.exit();
                    }
                    WindowEvent::Resized(size) => {
                        state.resize(size);
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        if event.state == ElementState::Pressed {
                            if state.modifiers.control_key() {
                                if let Key::Character(ch) = &event.logical_key {
                                    let mut chars = ch.chars();
                                    if let Some(ch) = chars.next() {
                                        match ch.to_ascii_lowercase() {
                                            'c' | 'v' => {
                                                return;
                                            }
                                            _ => {}
                                        }
                                        if let Some(code) = control_code_for_char(ch) {
                                            if code == 0x03 {
                                                state.input_len = 0;
                                            }
                                            state.send_input_bytes(&[code]);
                                        }
                                    }
                                }
                                return;
                            }
                            if let Key::Named(key) = event.logical_key {
                                state.handle_special_key(key);
                            }
                        }
                        if let Some(text) = event.text.as_ref() {
                            state.handle_input_text(text);
                        }
                    }
                    WindowEvent::ModifiersChanged(modifiers) => {
                        state.modifiers = modifiers.state();
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let lines = match delta {
                            winit::event::MouseScrollDelta::LineDelta(_, y) => y.round() as i32,
                            winit::event::MouseScrollDelta::PixelDelta(pos) => {
                                if pos.y > 0.0 {
                                    1
                                } else if pos.y < 0.0 {
                                    -1
                                } else {
                                    0
                                }
                            }
                        };
                        if lines != 0 && state.screen.scroll_view(lines) {
                            state.window.request_redraw();
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        state.render();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                state.check_pty_status();
                state.drain_font_download();
                state.update_cursor_blink();
                if state.exit_requested {
                    target.exit();
                    return;
                }
                if state.pty_closed {
                    target.exit();
                    return;
                }
                state.window.request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}

#[cfg(windows)]
fn apply_taskbar_icon_resource(window: &winit::window::Window) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED,
        WM_SETICON,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let hwnd = match handle.as_raw() {
        RawWindowHandle::Win32(handle) => handle.hwnd.get(),
        _ => return,
    };
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if hinstance == 0 {
        return;
    }

    let icon = unsafe {
        LoadImageW(
            hinstance,
            1usize as *const u16,
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
    };
    if icon == 0 {
        return;
    }

    unsafe {
        let _ = SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
        let _ = SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
    }
}

fn build_terminal_icon() -> Option<winit::window::Icon> {
    let size = 32u32;
    let rgba = make_terminal_icon_rgba(size, size);
    winit::window::Icon::from_rgba(rgba, size, size).ok()
}

fn make_terminal_icon_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut buffer = vec![0u8; (width * height * 4) as usize];
    let bg = [12u8, 16u8, 22u8, 255u8];
    fill_rect(&mut buffer, width, 0, 0, width, height, bg);

    let bar = [20u8, 26u8, 34u8, 255u8];
    let bar_height = (height / 5).max(8);
    fill_rect(&mut buffer, width, 0, 0, width, bar_height, bar);

    let accent = [80u8, 160u8, 255u8, 255u8];
    let accent_size = (height / 10).max(4);
    let accent_y = (bar_height / 2).saturating_sub(accent_size / 2);
    let accent_gap = accent_size + (accent_size / 2).max(2);
    fill_rect(&mut buffer, width, accent_size, accent_y, accent_size, accent_size, accent);
    fill_rect(
        &mut buffer,
        width,
        accent_size + accent_gap,
        accent_y,
        accent_size,
        accent_size,
        [60u8, 210u8, 120u8, 255u8],
    );
    fill_rect(
        &mut buffer,
        width,
        accent_size + accent_gap * 2,
        accent_y,
        accent_size,
        accent_size,
        [255u8, 208u8, 90u8, 255u8],
    );

    let prompt = [220u8, 240u8, 250u8, 255u8];
    let glyph_scale = (height / 12).max(3);
    draw_glyph(
        &mut buffer,
        width,
        width / 4,
        height / 3,
        &GLYPH_GT,
        glyph_scale,
        prompt,
    );
    draw_glyph(
        &mut buffer,
        width,
        width / 2,
        height / 3 + glyph_scale,
        &GLYPH_UNDERSCORE,
        glyph_scale,
        prompt,
    );

    buffer
}

fn fill_rect(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_w: u32,
    rect_h: u32,
    color: [u8; 4],
) {
    let max_x = (x + rect_w).min(width);
    for py in y..(y + rect_h) {
        for px in x..max_x {
            set_pixel(buffer, width, px, py, color);
        }
    }
}

fn draw_glyph(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    glyph: &[&str; 7],
    scale: u32,
    color: [u8; 4],
) {
    for (row, line) in glyph.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != '1' {
                continue;
            }
            let px = x + col as u32 * scale;
            let py = y + row as u32 * scale;
            fill_rect(buffer, width, px, py, scale, scale, color);
        }
    }
}

fn set_pixel(buffer: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let idx = (y * width + x) as usize * 4;
    if idx + 4 <= buffer.len() {
        buffer[idx..idx + 4].copy_from_slice(&color);
    }
}

const GLYPH_GT: [&str; 7] = [
    "10000", "01000", "00100", "01000", "10000", "00000", "00000",
];
const GLYPH_UNDERSCORE: [&str; 7] = [
    "00000", "00000", "00000", "00000", "00000", "11111", "00000",
];
