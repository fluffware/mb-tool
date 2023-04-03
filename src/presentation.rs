#[derive(Debug)]
pub struct Presentation {
    pub radix: u8,
    pub decimals: u8,
    pub scale: f32,
    pub unit: Option<String>,
}
