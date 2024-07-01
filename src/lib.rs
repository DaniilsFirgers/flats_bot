pub mod asynchronous;
pub mod flats;
pub mod logger;
pub mod telegram;

use core::panic;
use std::sync::Arc;

use dotenv::dotenv;
use logger::Logger;
use tokio::{signal, sync::Mutex};

pub fn init() -> Result<(), anyhow::Error> {
    if let Err(error) = Logger::new() {
        return Err(error);
    }
    dotenv().ok();
    let dotenv_qwe = dotenv::var("TELEGRAM_BOT_TOKEN");
    println!("dotenv_qwe: {:?}", dotenv_qwe);
    Logger::info("Logger initialized successfully");
    let tokio_runtime = Arc::new(asynchronous::tokio::runtime::AppRuntime::new());
    let flats_parser = flats::FlatsParser::new(Arc::clone(&tokio_runtime));

    let mut telegram_bot = telegram::FlatsBotTelegram::new(
        Arc::clone(&tokio_runtime),
        Arc::new(Mutex::new(flats_parser)),
    );
    telegram_bot.init()?;

    let bot_tokio = Arc::clone(&tokio_runtime);
    bot_tokio.runtime.spawn(async move {
        let _qwe = telegram_bot.run().await;
    });

    let blocking_tokio = Arc::clone(&tokio_runtime);
    blocking_tokio.runtime.block_on(async {
        if let Err(res) = signal::ctrl_c().await {
            Logger::error(&format!("Failed to catch ctrl-c signal: {}", res));
            panic!("Failed to catch ctrl-c signal: {}", res);
        };
    });

    Ok(())
}
