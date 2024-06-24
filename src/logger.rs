use anyhow::Error;
use log4rs;

pub struct Logger {
    pub log: log4rs::Handle,
}

impl Logger {
    pub fn new() -> Result<(), Error> {
        log4rs::init_file("log4rs.yaml", Default::default())?;
        Ok(())
    }

    pub fn error(message: &str) {
        log::error!(target: "base", "{message}");
    }

    pub fn warn(message: &str) {
        log::warn!(target: "base", "{message}");
    }

    pub fn info(message: &str) {
        log::info!(target: "base", "{message}");
    }

    pub fn debug(message: &str) {
        log::debug!(target: "base", "{message}");
    }
}
