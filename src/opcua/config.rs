/// Runtime config passed to OpcUaSource::new.
#[derive(Debug, Clone)]
pub struct OpcUaConfig {
    /// OPC UA server endpoint URL, e.g. "opc.tcp://localhost:4840"
    pub endpoint_url: String,
    /// Node ID strings to monitor.
    pub node_ids: Vec<String>,
    /// How often to poll the node, in milliseconds.
    pub poll_interval_ms: u64,
    /// Optional username for UserName identity token (empty = Anonymous).
    pub username: String,
    /// Optional password for UserName identity token.
    pub password: String,
}

impl Default for OpcUaConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "opc.tcp://localhost:4840".into(),
            node_ids: vec![],
            poll_interval_ms: 1000,
            username: String::new(),
            password: String::new(),
        }
    }
}
