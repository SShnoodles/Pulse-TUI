/// IEC 60870-5-104 client connection settings.
#[derive(Debug, Clone)]
pub struct Iec104Config {
    pub host: String,
    pub port: u16,
    pub common_address: u16,
    pub originator_address: u8,
}

impl Default for Iec104Config {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 2404,
            common_address: 1,
            originator_address: 0,
        }
    }
}
