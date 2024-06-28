use crate::ParseError;

use crate::evdev_mappings;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct KeyAttribute: u8 {
        const None = 0b00000000;
        const Send = 0b00000001;
        const OnRelease = 0b00000010;
        const Both = Self::Send.bits() | Self::OnRelease.bits();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierRepr(pub String);

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum Modifier {
    Super,
    Alt,
    Altgr,
    Control,
    Shift,
    Any,
}

impl From<ModifierRepr> for Modifier {
    fn from(value: ModifierRepr) -> Self {
        match value.0.to_lowercase().as_str() {
            "ctrl" => Modifier::Control,
            "control" => Modifier::Control,
            "super" | "mod4" | "meta" => Modifier::Super,
            "alt" => Modifier::Alt,
            "mod1" => Modifier::Alt,
            "altgr" => Modifier::Altgr,
            "mod5" => Modifier::Altgr,
            "shift" => Modifier::Shift,
            "any" => Modifier::Any,
            _ => panic!("that's not a modifier"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub key: evdev::Key,
    pub attribute: KeyAttribute,
}

impl Key {
    pub fn new(key: evdev::Key, attribute: KeyAttribute) -> Self {
        Self { key, attribute }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRepr {
    pub key: String,
    pub attribute: KeyAttribute,
}

impl TryFrom<KeyRepr> for Key {
    type Error = ParseError;

    fn try_from(value: KeyRepr) -> Result<Self, Self::Error> {
        let key = evdev_mappings::convert(&value.key)?;
        let attribute = value.attribute;

        Ok(Self { key, attribute })
    }
}
