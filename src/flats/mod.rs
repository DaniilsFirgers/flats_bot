use crate::{asynchronous::tokio::runtime::AppRuntime, logger};
use logger::Logger;
use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::os::linux::raw;
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct CategoryStructure {
    pub name: String,
    pub href: String,
    pub deal_types: Vec<String>,
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

pub struct Flat {
    pub street_name: String,
    pub price: String,
    pub url: String,
    pub image_url: String,
    pub rooms: u32,
    pub square_meters: u32,
    pub floor: u32,
    pub series: String,
}

pub struct FlatCriteria {
    pub href: String,
    pub city: String,
    pub district: String,
    pub deal_type: String,
    pub price_from: u32,
    pub price_to: u32,
    // pub rooms_from: u32,
    // pub rooms_to: u32,
    // pub square_meters_from: u32,
    // pub square_meters_to: u32,
    // pub floor_from: u32,
    // pub floor_to: u32,
    // pub series: String,
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

    pub async fn parse_global_data(&mut self) -> Result<(), anyhow::Error> {
        let Ok(regex) = Regex::new(r"\u{a0}") else {
            Logger::info("Failed to create regex");
            return Err(anyhow::anyhow!("Failed to create regex"));
        };

        let full_url = format!("{}/lv/real-estate/flats/", self.url_base);
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
            let city_name = element.text().collect::<String>();
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
                let Some(district_href) = district.value().attr("href") else {
                    Logger::info(format!("Failed to get href attribute from {:?}", district_name).as_str());
                    continue;
                };

                let full_deal_types_url = format!("{}{}", self.url_base, district_href);
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
                let Ok(deal_types_selector) = Selector::parse("select.filter_sel.l100 > option") else {
                    Logger::info("Failed to parse selector");
                    return Err(anyhow::anyhow!("Failed to parse selector"));
                };

                let deal_types = deal_types_html
                    .select(&deal_types_selector)
                    .collect::<Vec<ElementRef>>();

                let deal_types: Vec<String> = deal_types
                    .iter()
                    .map(|deal_type| {
                        let deal_type = deal_type.text().collect::<String>();
                        let cleaned_deal_type = regex.replace_all(&deal_type, "");
                        cleaned_deal_type.to_string()
                    })
                    .collect();

                districts_set.insert(CategoryStructure {
                    name: district_name.to_string(),
                    href: district_href.to_string(),
                    deal_types,
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

    pub async fn parse_flats_by_criteria(
        &self,
        flat_criteria: FlatCriteria,
    ) -> Result<(), anyhow::Error> {
        let full_url = format!("{}{}/page1.html", self.url_base, flat_criteria.href);
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
            Logger::info(format!("Failed to get response from {}: {}", full_url, error).as_str());
            return Err(anyhow::anyhow!(
                "Failed to get response from {}: {}",
                full_url,
                error
            ));
        }
        let raw_html = raw_html?;
        let document = Html::parse_document(&raw_html);

        let Ok(table_selector) = Selector::parse("form#filter_frm>table>tbody") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        };

        let Ok(pages_selector) = Selector::parse("form#filter_frm div.td2") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        };

        let Ok(page_index_selector) = Selector::parse("a") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        };

        // only one page of flats
        if document.select(&pages_selector).next().is_none() {}

        let page_selector = document.select(&pages_selector).next();
        if page_selector.is_none() {
            return Err(anyhow::anyhow!("Failed to get page selector"));
        }
        let page_selector = page_selector.unwrap();

        let Some(tbody_element) = document.select(&table_selector).nth(1) else {
            Logger::info("Failed to get tbody element");
            return Err(anyhow::anyhow!("Failed to get tbody element"));
        };

        let Ok(tr_element) = Selector::parse("tr") else {
            Logger::info("Failed to parse selector");
            return Err(anyhow::anyhow!("Failed to parse selector"));
        
        };
        let tr_elements = tbody_element.select(&tr_element).collect::<Vec<ElementRef>>();
        for (index, tr_element) in tr_elements.iter().enumerate() {
            let num_rows = tr_elements.len();
            if index == 0 || index == num_rows - 1 {
                continue; // Skip the first and last rows
            }
            println!("element: {:?}", tr_element);

        }

        Ok(())
    }
}
