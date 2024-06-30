use crate::{asynchronous::tokio::runtime::AppRuntime, logger};
use logger::Logger;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct CategoryStructure {
    name: String,
    href: String,
}
#[derive(Debug, Eq, PartialEq)]
pub struct City {
    name: String,
    href: String,
    districts: HashSet<CategoryStructure>,
}
impl Hash for City {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.href.hash(state);
        for district in &self.districts {
            district.hash(state);
        }
    }
}

pub struct FlatsParser {
    tokio: Arc<AppRuntime>,
    pub cities: HashSet<City>,
    url_base: String,
    request_client: Client,
}

impl FlatsParser {
    pub fn new(tokio: Arc<AppRuntime>) -> Self {
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

    pub fn parse_cities_and_districts(&mut self) -> Result<(), anyhow::Error> {
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

        let Ok(h4_selector) = Selector::parse("a.a_category") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        };

        // Select all matching h4 elements
        let cities: Vec<ElementRef> = html.select(&h4_selector).collect();
        let mut cities_href_map: HashMap<String, String> = HashMap::new(); // <city_name, city_href>
        for element in cities {
            let city_name = element.text().collect::<String>();
            let Some(city_href) = element.value().attr("href") else {
                Logger::info(format!("Failed to get href attribute for {:?}", city_name).as_str());
                continue;
            };
            cities_href_map.insert(city_name, city_href.to_string());
        }

        // make requests to get districts for each city
        for (city_name, city_href) in cities_href_map {
            let full_url = format!("{}{}", self.url_base, city_href);
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

            let Ok(href_selector) = Selector::parse("a.a_category") else {
                Logger::info("Failed to parse selector");
                return Err(anyhow::anyhow!("Failed to parse selector"));
            };

            let districts: Vec<ElementRef> = html.select(&href_selector).collect();
            let mut districts_set: HashSet<CategoryStructure> = HashSet::new();
            for district in districts {
                let district_name = district.text().collect::<String>();
                let Some(district_href) = district.value().attr("href") else {
                    Logger::info(format!("Failed to get href attribute from {:?}", district_name).as_str());
                    continue;
                };
                districts_set.insert(CategoryStructure {
                    name: district_name,
                    href: district_href.to_string(),
                });
            }
            self.cities.insert(City {
                name: city_name,
                href: city_href,
                districts: districts_set,
            });
        }
        Ok(())
    }
}
