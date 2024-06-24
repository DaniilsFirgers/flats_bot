use crate::asynchronous::tokio::runtime::AppRuntime;
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::HashSet;

pub struct CategoryStructure {
    name: String,
    href: String,
}
pub struct City {
    name: String,
    href: String,
    districts: HashSet<CategoryStructure>,
}
pub struct FlatsParser {
    tokio: AppRuntime,
    cities: HashSet<City>,
    url_base: String,
    request_client: Client,
}

impl FlatsParser {
    pub fn new(tokio: AppRuntime) -> Self {
        let cities: HashSet<City> = HashSet::new();
        let url_base = String::from("https://www.ss.com");
        let request_client = Client::new();
        Self {
            tokio,
            cities,
            url_base,
            request_client,
        }
    }

    pub fn parse_cities_and_districts(&self) -> Result<(), anyhow::Error> {
        let full_url = format!("{}/en/real-estate/flats/", self.url_base);
        let raw_html = self.tokio.runtime.block_on(async {
            let res = self.request_client.get(&full_url).send().await?;
            if !res.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to get successful response from {}",
                    full_url
                ));
            }
            let res = res.text().await?;
            Ok(res)
        })?;
        let html = Html::parse_document(&raw_html);
        let main_table = Selector::parse("div.top_head>table");
        if main_table.is_err() {
            return Err(anyhow::anyhow!("Failed to parse main table"));
        }
        let main_table = main_table.unwrap();
        println!("Main table: {:?}", main_table);
        let cities = html.select(&main_table).next();
        println!("Cities: {:?}", cities);

        Ok(())
    }
}
