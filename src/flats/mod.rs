use crate::{asynchronous::tokio::runtime::AppRuntime, logger};
use logger::Logger;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct CategoryStructure {
    pub name: String,
    pub href: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct City {
    pub name: String,
    pub href: String,
    pub districts: HashSet<CategoryStructure>,
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
    pub deal_types: Vec<String>,
    url_base: String,
    request_client: Client,
}

impl FlatsParser {
    pub fn new(tokio: Arc<AppRuntime>) -> Self {
        let cities: HashSet<City> = HashSet::new();
        let url_base = String::from("https://www.ss.com");
        let request_client = Client::new();
        let deal_types: Vec<String> = Vec::new();
        Self {
            tokio,
            cities,
            deal_types,
            url_base,
            request_client,
        }
    }

    pub async fn parse_cities_and_districts(&mut self) -> Result<(), anyhow::Error> {
        let full_url = format!("{}/en/real-estate/flats/", self.url_base);
        let raw_html: Result<String, anyhow::Error> = {
            let res = self.request_client.get(&full_url).send().await?;
            if !res.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to get successful response from {}",
                    full_url
                ));
            }
            let res = res.text().await?;
            Ok(res)
        };
        let raw_html = raw_html?;
        let html = Html::parse_document(&raw_html);

        let Ok(h4_selector) = Selector::parse("a.a_category") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        };

        // Select all matching h4 elements
        let cities: Vec<ElementRef> = html.select(&h4_selector).collect();
        let mut cities_href_map: HashMap<String, String> = HashMap::new(); // <city_name, city_href>
        for element in cities {
            let mut city_name = element.text().collect::<String>();
            let region = city_name.split_whitespace().find(|&word| word.eq("and"));
            if region.is_some() && city_name.split_whitespace().next().is_some() {
                city_name = city_name.split_whitespace().next().unwrap().to_string();
            }

            let Some(city_href) = element.value().attr("href") else {
                Logger::info(format!("Failed to get href attribute for {:?}", city_name).as_str());
                continue;
            };
            cities_href_map.insert(city_name.to_string(), city_href.to_string());
        }

        // make requests to get districts for each city
        for (city_name, city_href) in cities_href_map {
            let full_url = format!("{}{}", self.url_base, city_href);
            let raw_html: Result<String, anyhow::Error> = {
                let res = self.request_client.get(&full_url).send().await?;
                if !res.status().is_success() {
                    return Err(anyhow::anyhow!(
                        "Failed to get successful response from {}",
                        full_url
                    ));
                }
                let res = res.text().await?;
                Ok(res)
            };
            if let Err(error) = raw_html {
                Logger::info(
                    format!("Failed to get response from {}: {}", full_url, error).as_str(),
                );
                continue;
            }

            let html = Html::parse_document(&raw_html.unwrap());

            let Ok(href_selector) = Selector::parse("a.a_category") else {
                Logger::info("Failed to parse selector");
                return Err(anyhow::anyhow!("Failed to parse selector"));
            };

            let districts: Vec<ElementRef> = html.select(&href_selector).collect();
            let mut districts_set: HashSet<CategoryStructure> = HashSet::new();
            for district in districts {
                let district_name = district.text().collect::<String>();
                let Some(district_name) = district_name.split_whitespace().next() else {
                    Logger::info("Failed to get city name");
                    continue;
                };
                let Some(district_href) = district.value().attr("href") else {
                    Logger::info(format!("Failed to get href attribute from {:?}", district_name).as_str());
                    continue;
                };
                districts_set.insert(CategoryStructure {
                    name: district_name.to_string(),
                    href: district_href.to_string(),
                });
            }
            self.cities.insert(City {
                name: city_name,
                href: city_href,
                districts: districts_set,
            });
        }

        // parse deal types here
        let random_city = self.cities.iter().next();
        let Some(random_city) = random_city else {
            Logger::info("Failed to get random city");
            return Err(anyhow::anyhow!("Failed to get random city"));
        };

        let random_city_random_district = random_city.districts.iter().next();
        let Some(random_district) = random_city_random_district else {
            Logger::info("Failed to get random city random district");
            return Err(anyhow::anyhow!("Failed to get random city random district"));
        };

        let full_deal_types_url = format!("{}{}", self.url_base, random_district.href);
        let raw_deal_types_html: Result<String, anyhow::Error> = {
            let res = self.request_client.get(&full_deal_types_url).send().await?;
            if !res.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to get successful response from {}",
                    full_deal_types_url
                ));
            }
            let res = res.text().await?;
            Ok(res)
        };

        if let Err(error) = raw_deal_types_html {
            Logger::info(
                format!(
                    "Failed to get response from {}: {}",
                    full_deal_types_url, error
                )
                .as_str(),
            );
            return Err(anyhow::anyhow!(
                "Failed to get response from {}: {}",
                full_deal_types_url,
                error
            ));
        }
        let deal_types_html = Html::parse_document(&raw_deal_types_html.unwrap());

        println!("deal types html{:?}", deal_types_html);

        let Ok(deal_types_selector) = Selector::parse("select.filter_sel l100") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        };
        let deal_types: Vec<ElementRef> = deal_types_html.select(&deal_types_selector).collect();
        println!("deal types{:?}", deal_types);
        Ok(())
    }
}
