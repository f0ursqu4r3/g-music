use tracing_subscriber::EnvFilter;

pub fn init() {
    let default_filter = if cfg!(debug_assertions) {
        "gmusic_lib=debug,gmusic=debug"
    } else {
        "gmusic_lib=info,gmusic=info"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    if let Err(error) = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init()
    {
        eprintln!("failed to initialize Rust logging: {error}");
    }
}
