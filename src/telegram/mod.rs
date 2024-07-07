use std::sync::Arc;

use crate::asynchronous::tokio::runtime::AppRuntime;
use crate::{flats::FlatsParser, logger::Logger};
use dptree::case;
use teloxide::dispatching::{dialogue, UpdateHandler};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*, utils::command::BotCommands};

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
    ReceiveCityName,
    ReceiveDistrictName {
        city_name: String,
    },
    ReceiveDealType {
        city_name: String,
        district_name: String,
    },
}
#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Start the dialogue.")]
    Start,
    #[command(description = "Get help.")]
    Help,
    #[command(description = "Cancel the dialogue.")]
    Cancel,
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

    fn create_schema(&self) -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
        let command_handler = teloxide::filter_command::<Command, _>()
            .branch(
                case![State::Start]
                    .branch(case![Command::Help].endpoint(Self::help_message))
                    .branch(case![Command::Start].endpoint(Self::start)),
            )
            .branch(case![Command::Cancel].endpoint(Self::cancel));

        let message_handler = Update::filter_message()
            .branch(command_handler)
            .branch(dptree::case![State::ReceiveCityName].endpoint(Self::recieve_city_name))
            .branch(
                dptree::case![State::ReceiveDistrictName { city_name }]
                    .endpoint(Self::receive_district_name),
            )
            .branch(
                dptree::case![State::ReceiveDealType {
                    city_name,
                    district_name
                }]
                .endpoint(Self::receive_deal_type),
            )
            .branch(dptree::entry().endpoint(Self::unhandled_message));

        dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler)
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        let dependencies = Arc::new(BotDependencies {
            flats_parser: self.flats_parser.clone(),
        });

        Dispatcher::builder(self.bot.clone(), self.create_schema())
            .dependencies(dptree::deps![dependencies, InMemStorage::<State>::new()])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    async fn help_message(bot: Bot, msg: Message) -> HandlerResult {
        let help_text = Command::descriptions();
        bot.send_message(msg.chat.id, help_text.to_string()).await?;
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

        let greeting_message = format!("Let's start! Please select a city: \n\n{}", cities);
        bot.send_message(msg.chat.id, greeting_message).await?;
        dialogue.update(State::ReceiveCityName).await?;
        Ok(())
    }

    async fn recieve_city_name(
        dependencies: Arc<BotDependencies>,
        bot: Bot,
        dialogue: MyDialogue,
        msg: Message,
    ) -> HandlerResult {
        let Some(city_name): Option<&str> = msg.text() else {
            bot.send_message(msg.chat.id, "Message should be a plain text").await?;
            return Ok(())
        };
        let flats_parser = dependencies.flats_parser.lock().await;
        let city_info = flats_parser
            .cities
            .iter()
            .find(|city| city.name.eq(city_name));
        let Some(city_info) = city_info else {
            bot.send_message(msg.chat.id, format!("City '{}' name is invalid. Please try again!", city_name)).await?;
            return Ok(())
        };
        let mut districts = city_info
            .districts
            .iter()
            .map(|district| format!("• {}", district.name))
            .collect::<Vec<_>>();
        districts.sort();
        let districts = districts.join("\n");
        bot.send_message(
            msg.chat.id,
            format!("Please select a district: \n\n{}", districts),
        )
        .await?;
        dialogue
            .update(State::ReceiveDistrictName {
                city_name: city_name.into(),
            })
            .await?;

        Ok(())
    }

    async fn receive_district_name(
        dependencies: Arc<BotDependencies>,
        bot: Bot,
        dialogue: MyDialogue,
        city_name: String, // Available from `State::ReceiveAge`.
        msg: Message,
    ) -> HandlerResult {
        let Some(district_name): Option<&str> = msg.text() else {
            bot.send_message(msg.chat.id, "Message should be a plain text").await?;
            return Ok(())
        };
        // here i should scrape deal types
        let flats_parser = dependencies.flats_parser.lock().await;
        let city_option = flats_parser
            .cities
            .iter()
            .find(|city| city.name.eq(&city_name));

        let Some(city) = city_option else {
        bot.send_message(msg.chat.id, "City not found").await?;
        return Ok(());
        };
        let city = city.clone();
        let Some(district) = city.districts.iter().find(|district| district.name.eq(district_name)) else {
            bot.send_message(msg.chat.id, "District not found").await?;
            return Ok(())
        };
        let deal_options = district
            .deal_types
            .iter()
            .map(|deal_type| format!("• {}", deal_type))
            .collect::<Vec<_>>()
            .join("\n");

        bot.send_message(
            msg.chat.id,
            format!("Please select a deal type: \n\n{}", deal_options),
        )
        .await?;

        dialogue
            .update(State::ReceiveDealType {
                city_name: city_name.into(),
                district_name: district_name.into(),
            })
            .await?;

        Ok(())
    }

    async fn receive_deal_type(
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

    async fn cancel(bot: Bot, msg: Message) -> HandlerResult {
        bot.send_message(msg.chat.id, "Unhandled message: Please follow the prompts.")
            .await?;
        Ok(())
    }

    async fn unhandled_message(bot: Bot, msg: Message) -> HandlerResult {
        let help_text = Command::descriptions();
        bot.send_message(
            msg.chat.id,
            format!("Unhandled message. Available commands:\n{}", help_text),
        )
        .await?;
        Ok(())
    }
}
