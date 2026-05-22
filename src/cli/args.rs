use clap::Parser;

/// Real-time terminal monitor — MQTT and beyond
#[derive(Parser, Debug)]
#[command(name = "pulse", version)]
pub struct Args {
    /// Broker host
    #[arg(short, long, default_value = "localhost")]
    pub broker: String,

    /// Broker port
    #[arg(short, long, default_value_t = 1883)]
    pub port: u16,

    /// Topics to subscribe (repeat for multiple: -t a -t b)
    #[arg(short, long)]
    pub topics: Vec<String>,

    /// MQTT client ID
    #[arg(long, default_value = "pulse-tui")]
    pub client_id: String,
}
