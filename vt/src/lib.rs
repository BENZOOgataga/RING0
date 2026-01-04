#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VtEvent {
    Print(char),
    Newline,
    CarriageReturn,
    Backspace,
}

pub struct VtParser;

impl VtParser {
    pub fn new() -> Self {
        Self
    }

    pub fn advance(&mut self, input: &[u8], events: &mut Vec<VtEvent>) {
        for byte in input {
            match byte {
                b'\n' => events.push(VtEvent::Newline),
                b'\r' => events.push(VtEvent::CarriageReturn),
                0x08 => events.push(VtEvent::Backspace),
                0x20..=0x7E => events.push(VtEvent::Print(*byte as char)),
                _ => {}
            }
        }
    }
}

impl Default for VtParser {
    fn default() -> Self {
        Self::new()
    }
}
