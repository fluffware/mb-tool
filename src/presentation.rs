
#[derive(Debug)]
pub struct Presentation {
    pub base: u8,
    pub decimals: u8,
    pub scale: f32,
    pub unit: Option<String>,
}