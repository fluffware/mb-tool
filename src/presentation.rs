#[derive(Debug)]
pub enum DisplayType {
    Integer{signed: bool, base: u8},
    Float{decimals: u8},
    String{fill: char},
}

#[derive(Debug)]
pub struct Presentation {
    pub display: DisplayType,
    pub scale: f32,
    pub unit: Option<String>,
}