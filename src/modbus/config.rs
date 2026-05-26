pub struct ModbusConfig {
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub poll_interval_ms: u64,
}

impl Default for ModbusConfig {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 502,
            unit_id: 1,
            poll_interval_ms: 1000,
        }
    }
}
