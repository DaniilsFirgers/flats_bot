use tokio::runtime::{Builder, Runtime};

pub struct AppRuntime {
    pub runtime: Runtime,
}

impl AppRuntime {
    pub fn new() -> Self {
        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        AppRuntime { runtime }
    }
}
