#[derive(defmt::Format, Clone, Copy)]
pub enum Message {
    QuerySignal,
    Signal(crate::Signal),
}

impl Message {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != MSG_SIZE || bytes[0] != HEADER || bytes[3] != FOOTER {
            return None;
        }
        let command = bytes[1];
        let payload = bytes[2];

        match command {
            0 => Some(Self::QuerySignal),
            1 => Some(Self::Signal(crate::Signal::from_u8(payload)?)),
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> [u8; MSG_SIZE] {
        let mut bytes = [HEADER, 0, 0, FOOTER];
        match self {
            Self::QuerySignal => bytes[1] = 0,
            Self::Signal(signal) => {
                bytes[1] = 1;
                bytes[2] = *signal as u8;
            }
        }

        bytes
    }
}

pub const MSG_SIZE: usize = 4;
const HEADER: u8 = 117;
const FOOTER: u8 = 255;
