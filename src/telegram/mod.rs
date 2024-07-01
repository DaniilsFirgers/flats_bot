use std::sync::Arc;

use crate::asynchronous::tokio::runtime::AppRuntime;
use crate::{flats::FlatsParser, logger::Logger};
use teloxide::types::{ChatId, ParseMode};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};
use tokio::sync::Mutex;

pub struct FlatsBotTelegram {
    pub flats_parser: Arc<Mutex<FlatsParser>>,
    tokio_runtime: Arc<AppRuntime>,
    bot: Bot,
}
struct BotDependencies {
    flats_parser: Arc<Mutex<FlatsParser>>,
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default, Debug)]
pub enum State {
    #[default]
    Start,
    ReceiveFullName,
    ReceiveAge {
        full_name: String,
    },
    ReceiveLocation {
        full_name: String,
        age: u8,
    },
}

impl FlatsBotTelegram {
    pub fn new(tokio_runtime: Arc<AppRuntime>, flats_parser: Arc<Mutex<FlatsParser>>) -> Self {
        let bot = Bot::from_env();
        Self {
            tokio_runtime,
            flats_parser,
            bot,
        }
    }

    pub fn init(&mut self) -> Result<(), anyhow::Error> {
        let cities_parsing_res: Result<(), anyhow::Error> =
            self.tokio_runtime.runtime.block_on(async {
                let mut parser = self.flats_parser.lock().await;
                parser.parse_cities_and_districts().await?;
                Ok(())
            });
        cities_parsing_res?;
        Logger::info("Cities and districts parsed successfully");
        Ok(())
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        let dependencies = Arc::new(BotDependencies {
            flats_parser: self.flats_parser.clone(),
        });
        Dispatcher::builder(
            self.bot.clone(),
            Update::filter_message()
                .enter_dialogue::<Message, InMemStorage<State>, State>()
                .branch(dptree::case![State::Start].endpoint(Self::start))
                .branch(dptree::case![State::ReceiveFullName].endpoint(Self::receive_full_name))
                .branch(dptree::case![State::ReceiveAge { full_name }].endpoint(Self::receive_age))
                .branch(
                    dptree::case![State::ReceiveLocation { full_name, age }]
                        .endpoint(Self::receive_location),
                ),
        )
        .dependencies(dptree::deps![dependencies, InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

        println!("Bot stopped");
        Ok(())
    }
    async fn start(
        dependencies: Arc<BotDependencies>,
        bot: Bot,
        dialogue: MyDialogue,
        msg: Message,
    ) -> HandlerResult {
        let flats_parser = dependencies.flats_parser.lock().await;
        let mut cities = flats_parser
            .cities
            .iter()
            .map(|city| format!("• {}", city.name))
            .collect::<Vec<_>>();

        cities.sort();
        let cities = cities.join("\n");

        println!("cities: {:?}", cities);
        let greeting_message = format!("Let's start! Please select a city: \n\n{}", cities);
        bot.send_message(msg.chat.id, greeting_message).await?;
        dialogue.update(State::ReceiveFullName).await?;
        println!("updated state");
        Ok(())
    }

    async fn receive_full_name(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
        println!("receive_full_name {:?}, dialogue: {:?}", msg, dialogue);
        match msg.text() {
            Some(text) => {
                bot.send_message(msg.chat.id, "How old are you?").await?;
                dialogue
                    .update(State::ReceiveAge {
                        full_name: text.into(),
                    })
                    .await?;
            }
            None => {
                bot.send_message(msg.chat.id, "Send me plain text.").await?;
            }
        }

        Ok(())
    }

    async fn receive_age(
        bot: Bot,
        dialogue: MyDialogue,
        full_name: String, // Available from `State::ReceiveAge`.
        msg: Message,
    ) -> HandlerResult {
        match msg.text().map(|text| text.parse::<u8>()) {
            Some(Ok(age)) => {
                bot.send_message(msg.chat.id, "What's your location?")
                    .await?;
                dialogue
                    .update(State::ReceiveLocation { full_name, age })
                    .await?;
            }
            _ => {
                bot.send_message(msg.chat.id, "Send me a number.").await?;
            }
        }

        Ok(())
    }

    async fn receive_location(
        bot: Bot,
        dialogue: MyDialogue,
        (full_name, age): (String, u8), // Available from `State::ReceiveLocation`.
        msg: Message,
    ) -> HandlerResult {
        match msg.text() {
            Some(location) => {
                let report = format!("Full name: {full_name}\nAge: {age}\nLocation: {location}");
                bot.send_message(msg.chat.id, report).await?;
                dialogue.exit().await?;
            }
            None => {
                bot.send_message(msg.chat.id, "Send me plain text.").await?;
            }
        }

        Ok(())
    }

    async fn catch_all(bot: Bot, msg: Message) -> HandlerResult {
        bot.send_message(msg.chat.id, "Unhandled message: Please follow the prompts.")
            .await?;
        Ok(())
    }
}
