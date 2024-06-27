pub mod asynchronous;
pub mod flats;
pub mod logger;
pub mod telegram;

use logger::Logger;

pub fn init() -> Result<(), anyhow::Error> {
    if let Err(error) = Logger::new() {
        return Err(error);
    }
    Logger::info("Logger initialized successfully");
    let tokio_runtime = asynchronous::tokio::runtime::AppRuntime::new();
    let mut flats_parser = flats::FlatsParser::new(tokio_runtime);
    if let Err(err) = flats_parser.parse_cities_and_districts() {
        return Err(err);
    }

    Ok(())
}
