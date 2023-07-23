use crate::types::{ForecastTime, WeatherProvider};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct HourlyUnits {
    time: String,
    temperature_2m: String,
}

#[derive(Deserialize, Debug)]
struct Hourly {
    time: Vec<String>,
    temperature_2m: Vec<f32>,
}

#[derive(Deserialize, Debug)]
struct Forecast {
    latitude: f32,
    longitude: f32,
    generationtime_ms: f32,
    utc_offset_seconds: u32,
    timezone: String,
    timezone_abbreviation: String,
    elevation: f32,
    hourly_units: HourlyUnits,
    hourly: Hourly,
}

#[derive(Deserialize, Debug)]
struct City {
    id: u32,
    name: String,
    latitude: f32,
    longitude: f32,
    elevation: f32,
    feature_code: String,
    country_code: String,
    #[serde(skip)]
    admin1_id: u32,
    #[serde(skip)]
    admin3_id: u32,
    #[serde(skip)]
    admin4_id: u32,
    timezone: String,
    #[serde(skip)]
    population: u32,
    #[serde(skip)]
    postcodes: Vec<String>,
    country_id: u32,
    country: String,
    #[serde(skip)]
    admin1: String,
    #[serde(skip)]
    admin3: String,
    #[serde(skip)]
    admin4: String,
}

#[derive(Deserialize, Debug)]
struct Geolocation {
    results: Option<Vec<City>>,
    generationtime_ms: f32,
}

#[derive(Clone)]
pub struct OpenMeteo {
    api_key: String,
    client: reqwest::Client,
    pub favourite_city: String,
}

impl OpenMeteo {
    pub fn new(api_key: String, default_city: String) -> Self {
        Self {
            api_key: api_key,
            client: reqwest::Client::new(),
            favourite_city: default_city,
        }
    }

    async fn get_geolocation(&self, city: String) -> Result<Option<(f32, f32)>> {
        let resp = self
            .client
            .get(format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json", city))
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await?
            .text()
            .await?;
        
        let data: Geolocation =
            serde_json::from_str(&resp).expect("problem with getting geolocation data");

        if let Some(results) = data.results {
            Ok(Some((results[0].latitude, results[0].longitude)))
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn get_forecast_url(lat: f32, long: f32, days: u32) -> String {
        format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m&forecast_days={}", lat.to_string(), long.to_string(), days.to_string())
    }
}

#[async_trait]
impl WeatherProvider for OpenMeteo {
    async fn get_temperature(&self, city: String) -> Option<f32> {
        if let Some(Some((lat, long))) = self.get_geolocation(city).await.ok() {
            let resp = if let Ok(req) = self
                .client
                .get(OpenMeteo::get_forecast_url(lat, long, 1))
                .header(CONTENT_TYPE, "application/json")
                .send()
                .await
            {
                req.text().await.ok()
            } else {
                return None;
            };

            if let Some(data) = resp {
                let hour = chrono::Local::now().hour();
                let forecast: Forecast = serde_json::from_str(&data).unwrap();
                return Some(forecast.hourly.temperature_2m[(hour - 1) as usize]);
            }
        }
        None
    }

    async fn get_temp_forecast(&self, city: String, time: ForecastTime) -> Option<f32> {
        todo!()
    }

    fn get_favourite_city(&self) -> String {
        self.favourite_city.clone()
    }
}
