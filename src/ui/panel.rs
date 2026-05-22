#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Panel {
    #[default]
    Topics,
    Messages,
}

impl Panel {
    pub fn toggle(self) -> Self {
        match self {
            Panel::Topics => Panel::Messages,
            Panel::Messages => Panel::Topics,
        }
    }
}
