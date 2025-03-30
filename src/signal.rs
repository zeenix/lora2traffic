#[derive(defmt::Format, Clone, Copy, PartialEq)]
#[repr(u8)]
pub(crate) enum Signal {
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
}

impl Default for Signal {
    fn default() -> Self {
        Self::Off
    }
}
