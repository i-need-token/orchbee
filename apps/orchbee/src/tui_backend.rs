use std::{
    io::{self, Write},
    sync::Arc,
};

use crossterm::{
    QueueableCommand,
    cursor::{Hide, MoveTo, Show},
    style::{
        Attribute, Color as CrosstermColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor,
    },
    terminal::{Clear, ClearType as CrosstermClearType},
};
use ratatui::{
    backend::{Backend, ClearType, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
    style::{Color, Modifier},
};

use crate::{
    terminal_device::WindowDimensions,
    terminal_lifecycle::{
        SYNCHRONIZED_UPDATE_BEGIN, SYNCHRONIZED_UPDATE_END, SynchronizedUpdateLease,
    },
    terminal_text::sanitize_terminal_symbol,
};

pub(crate) trait ResizableBackend: Backend {
    #[cfg(windows)]
    fn graphics_surface(&self) -> Option<crate::tui_graphics::Surface> {
        None
    }
    fn set_window_dimensions(&mut self, dimensions: WindowDimensions);
    fn graphics_probe_id(&self) -> Option<u32> {
        None
    }
}

pub(crate) struct FixedViewportBackend<W: Write> {
    #[cfg(windows)]
    graphics: Option<crate::tui_graphics::Surface>,
    graphics_probe_id: Option<u32>,
    cursor_position: Position,
    synchronized_update: Arc<SynchronizedUpdateLease>,
    window: WindowSize,
    writer: W,
}

impl<W: Write> FixedViewportBackend<W> {
    pub(crate) fn new(
        writer: W,
        dimensions: WindowDimensions,
        synchronized_update: Arc<SynchronizedUpdateLease>,
    ) -> Self {
        Self {
            #[cfg(windows)]
            graphics: None,
            graphics_probe_id: None,
            cursor_position: Position::ORIGIN,
            synchronized_update,
            window: to_window_size(dimensions),
            writer,
        }
    }
    #[cfg(windows)]
    pub(crate) fn enable_graphics_probe(&mut self, id: u32) {
        self.graphics_probe_id = (id != 0).then_some(id);
        self.graphics = self
            .graphics_probe_id
            .map(|id| crate::tui_graphics::Surface::new(id, Arc::clone(&self.synchronized_update)));
    }
}

pub(crate) struct RestorationGatedWriter<W: Write> {
    inner: W,
    synchronized_update: Arc<SynchronizedUpdateLease>,
}

impl<W: Write> RestorationGatedWriter<W> {
    pub(crate) fn new(inner: W, synchronized_update: Arc<SynchronizedUpdateLease>) -> Self {
        Self {
            inner,
            synchronized_update,
        }
    }
}

impl<W: Write> Write for RestorationGatedWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.synchronized_update.frame_output_blocked() {
            return Ok(buffer.len());
        }
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.synchronized_update.frame_output_blocked() {
            return Ok(());
        }
        self.inner.flush()
    }
}

impl<W: Write> ResizableBackend for FixedViewportBackend<W> {
    #[cfg(windows)]
    fn graphics_surface(&self) -> Option<crate::tui_graphics::Surface> {
        self.graphics.clone()
    }
    fn graphics_probe_id(&self) -> Option<u32> {
        self.graphics_probe_id
    }
    fn set_window_dimensions(&mut self, dimensions: WindowDimensions) {
        self.window = to_window_size(dimensions);
    }
}

impl<W: Write> Backend for FixedViewportBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        if !self.synchronized_update.frame_open() {
            self.synchronized_update.open_frame();
            self.writer.write_all(SYNCHRONIZED_UPDATE_BEGIN)?;
        }
        for (x, y, cell) in content {
            self.writer.queue(MoveTo(x, y))?;
            self.writer
                .queue(SetAttribute(Attribute::Reset))?
                .queue(SetForegroundColor(to_crossterm_color(cell.fg)))?
                .queue(SetBackgroundColor(to_crossterm_color(cell.bg)))?;
            queue_modifiers(&mut self.writer, cell.modifier)?;
            let symbol = sanitize_terminal_symbol(cell.symbol());
            self.writer.queue(Print(symbol.as_ref()))?;
            self.cursor_position = Position { x, y };
        }
        self.writer
            .queue(SetAttribute(Attribute::Reset))?
            .queue(ResetColor)?;
        Ok(())
    }

    fn append_lines(&mut self, count: u16) -> io::Result<()> {
        for _ in 0..count {
            self.writer.write_all(b"\n")?;
        }
        self.writer.flush()
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.writer.queue(Hide).map(drop)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.writer.queue(Show).map(drop)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor_position)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        self.writer.queue(MoveTo(position.x, position.y))?;
        self.cursor_position = position;
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        #[cfg(windows)]
        if clear_type == ClearType::All
            && let Some(graphics) = &self.graphics
        {
            graphics.invalidate();
        }
        let clear_type = match clear_type {
            ClearType::All => CrosstermClearType::All,
            ClearType::AfterCursor => CrosstermClearType::FromCursorDown,
            ClearType::BeforeCursor => CrosstermClearType::FromCursorUp,
            ClearType::CurrentLine => CrosstermClearType::CurrentLine,
            ClearType::UntilNewLine => CrosstermClearType::UntilNewLine,
        };
        self.writer.queue(Clear(clear_type)).map(drop)
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.window.columns_rows)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(self.window)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut first_error = None;
        #[cfg(windows)]
        let deleted = match &self.graphics {
            Some(graphics) => match graphics.write_frame(&mut self.writer, self.cursor_position) {
                Ok(deleted) => deleted,
                Err(error) => {
                    first_error = Some(error);
                    Vec::new()
                }
            },
            None => Vec::new(),
        };
        if self.synchronized_update.frame_open()
            && let Err(error) = self.writer.write_all(SYNCHRONIZED_UPDATE_END)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if let Err(error) = self.writer.flush()
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if first_error.is_none() {
            #[cfg(windows)]
            if let Some(graphics) = &self.graphics {
                graphics.flushed(&deleted);
            }
            self.synchronized_update.confirm_frame_closed();
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn queue_modifiers(writer: &mut impl Write, modifiers: Modifier) -> io::Result<()> {
    for (modifier, attribute) in [
        (Modifier::BOLD, Attribute::Bold),
        (Modifier::DIM, Attribute::Dim),
        (Modifier::ITALIC, Attribute::Italic),
        (Modifier::UNDERLINED, Attribute::Underlined),
        (Modifier::SLOW_BLINK, Attribute::SlowBlink),
        (Modifier::RAPID_BLINK, Attribute::RapidBlink),
        (Modifier::REVERSED, Attribute::Reverse),
        (Modifier::HIDDEN, Attribute::Hidden),
        (Modifier::CROSSED_OUT, Attribute::CrossedOut),
    ] {
        if modifiers.contains(modifier) {
            writer.queue(SetAttribute(attribute))?;
        }
    }
    Ok(())
}

const fn to_crossterm_color(color: Color) -> CrosstermColor {
    match color {
        Color::Reset => CrosstermColor::Reset,
        Color::Black => CrosstermColor::Black,
        Color::Red => CrosstermColor::DarkRed,
        Color::Green => CrosstermColor::DarkGreen,
        Color::Yellow => CrosstermColor::DarkYellow,
        Color::Blue => CrosstermColor::DarkBlue,
        Color::Magenta => CrosstermColor::DarkMagenta,
        Color::Cyan => CrosstermColor::DarkCyan,
        Color::Gray => CrosstermColor::Grey,
        Color::DarkGray => CrosstermColor::DarkGrey,
        Color::LightRed => CrosstermColor::Red,
        Color::LightGreen => CrosstermColor::Green,
        Color::LightYellow => CrosstermColor::Yellow,
        Color::LightBlue => CrosstermColor::Blue,
        Color::LightMagenta => CrosstermColor::Magenta,
        Color::LightCyan => CrosstermColor::Cyan,
        Color::White => CrosstermColor::White,
        Color::Rgb(red, green, blue) => CrosstermColor::Rgb {
            r: red,
            g: green,
            b: blue,
        },
        Color::Indexed(value) => CrosstermColor::AnsiValue(value),
    }
}

const fn to_window_size(dimensions: WindowDimensions) -> WindowSize {
    WindowSize {
        columns_rows: Size {
            width: dimensions.columns,
            height: dimensions.rows,
        },
        pixels: Size {
            width: dimensions.pixel_width,
            height: dimensions.pixel_height,
        },
    }
}

#[cfg(test)]
mod tests;
