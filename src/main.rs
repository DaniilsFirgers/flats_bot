fn main() {
    if let Err(err) = flats_bot::init() {
        panic!("Failed to initialize and run the bot: {}", err);
    }
}
