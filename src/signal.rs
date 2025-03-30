#[derive(defmt::Format, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Signal {
    Red = b'r',
    Yellow = b'y',
    Green = b'g',
    Off = b'o',
}

impl Signal {
    pub fn rotate(&mut self) {
        *self = match self {
            Self::Red => Self::Yellow,
            Self::Yellow => Self::Green,
            Self::Green => Self::Off,
            Self::Off => Self::Red,
        };
    }

    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            b'r' => Some(Self::Red),
            b'y' => Some(Self::Yellow),
            b'g' => Some(Self::Green),
            b'o' => Some(Self::Off),
            _ => None,
        }
    }
}

impl Default for Signal {
    fn default() -> Self {
        Self::Off
    }
}
