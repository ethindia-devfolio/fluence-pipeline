// src/lib.rs  

use marine_rs_sdk::marine;  
use marine_rs_sdk::module_manifest;  

module_manifest!();  

pub fn main() {}  

use serde::{Deserialize, Serialize};  
use std::env;  
use std::error::Error;  
use tokio::runtime::Runtime;  
use reqwest::Client;  
use serde_json::Value;  
use chrono::prelude::*;  

#[marine]  
#[derive(Debug, Serialize, Deserialize)]  
pub struct WeatherData {  
    city: String,  
    date: String,  
    temperature: f32,  
    humidity: u8,  
    wind_speed: f32,  
    weather_description: String,  
}  

#[marine]  
pub fn generate_weather_report(zip_code: String) -> String {  
    match call_openweathermap_api(&zip_code) {  
        Ok(weather_data) => format!(  
            "Current weather in {}:\n\  
            Temperature: {}°C\n\  
            Humidity: {}%\n\  
            Wind speed: {} m/s\n\  
            Weather description: {}\n\  
            Date: {}",  
            weather_data.city,  
            weather_data.temperature,  
            weather_data.humidity,  
            weather_data.wind_speed,  
            weather_data.weather_description,  
            weather_data.date,  
        ),  
        Err(e) => format!("Failed to connect to OpenWeatherMap API: {}", e),  
    }  
}  

fn call_openweathermap_api(zip_code: &str) -> Result<WeatherData, Box<dyn Error>> {  
    let api_key = get_env_api_key();  
    if api_key.is_empty() {  
        return Err("API key is not set in environment variable 'OPENWEATHERMAP_API_KEY'".into());  
    }  

    let base_url = format!(  
        "http://api.openweathermap.org/data/2.5/weather?zip={},us&appid={}&units=metric",  
        zip_code, api_key  
    );  

    let rt = Runtime::new()?;  
    let client = Client::new();  

    let res = rt.block_on(async {  
        let response = client.get(&base_url).send().await?;  
        if response.status().is_success() {  
            let json_data = response.json::<Value>().await?;  
            Ok(json_data)  
        } else {  
            Err(format!("Error response from API: {}", response.status()).into())  
        }  
    })?;  

    extract_relevant_data(res)  
}  

fn extract_relevant_data(data: Value) -> Result<WeatherData, Box<dyn Error>> {  
    let city_name = data["name"].as_str().ok_or("Missing city name")?.to_string();  
    let date = Local::today().format("%Y-%m-%d").to_string();  
    let temperature = data["main"]["temp"].as_f64().ok_or("Missing temperature")? as f32;  
    let humidity = data["main"]["humidity"].as_u64().ok_or("Missing humidity")? as u8;  
    let wind_speed = data["wind"]["speed"].as_f64().ok_or("Missing wind speed")? as f32;  
    let weather_description = data["weather"][0]["description"]  
        .as_str()  
        .ok_or("Missing weather description")?  
        .to_string();  

    Ok(WeatherData {  
        city: city_name,  
        date,  
        temperature,  
        humidity,  
        wind_speed,  
        weather_description,  
    })  
}  

fn get_env_api_key() -> String {  
    env::var("OPENWEATHERMAP_API_KEY").unwrap_or_else(|_| "".to_string())  
}