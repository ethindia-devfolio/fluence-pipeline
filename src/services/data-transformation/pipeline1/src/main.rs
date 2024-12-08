// src/lib.rs  

use marine_rs_sdk::marine;  
use marine_rs_sdk::module_manifest;  
use marine_rs_sdk::MountedBinaryResult;  

module_manifest!();  

pub fn main() {}  

use serde::{Deserialize, Serialize};  
use std::env;  
use std::fs;  
use std::path::PathBuf;  

use chrono::{Datelike, NaiveDate};  
use polars::prelude::*;  
use reqwest::Client;  
use std::error::Error;  
use tokio::runtime::Runtime;  

#[marine]  
#[derive(Debug, Serialize, Deserialize)]  
pub struct WeatherReport {  
    report: String,  
    min_temp: Vec<Option<f32>>,  
    max_temp: Vec<Option<f32>>,  
    wind_direction: String,  
    wind_speed: Vec<Option<f32>>,  
    rainfall: Vec<Option<f32>>,  
}  

#[marine]  
pub fn generate_weather_report(city: String, year: i32, month: u32) -> WeatherReport {  
    let file_path = "./weatherAUS.csv";  
    let system_prompt_path = "./system_prompt.txt";  
    let example_input_path = "./example_input1.txt";  
    let example_output_path = "./example_output1.txt";  

    // Load data  
    let df = match load_weather_data(file_path) {  
        Ok(df) => df,  
        Err(e) => {  
            return WeatherReport {  
                report: format!("Error loading data: {}", e),  
                min_temp: vec![],  
                max_temp: vec![],  
                wind_direction: "".to_string(),  
                wind_speed: vec![],  
                rainfall: vec![],  
            }  
        }  
    };  

    // Filter data  
    let filtered_df = get_city_month_data(df, &city, year, month);  

    if filtered_df.height() == 0 {  
        return WeatherReport {  
            report: format!("No data available for {} in {}/{}.", city, month, year),  
            min_temp: vec![],  
            max_temp: vec![],  
            wind_direction: "".to_string(),  
            wind_speed: vec![],  
            rainfall: vec![],  
        };  
    }  

    // Generate prompt  
    let prompt = generate_prompt(&filtered_df, &city, year, month);  

    // Call OpenAI API  
    let report = match call_openai_api(  
        &prompt,  
        system_prompt_path,  
        example_input_path,  
        example_output_path,  
    ) {  
        Ok(r) => r,  
        Err(e) => format!("Error in API call: {}", e),  
    };  

    // Extract weather data  
    let min_temp = get_min_temp(&filtered_df);  
    let max_temp = get_max_temp(&filtered_df);  
    let wind_direction = get_wind_direction(&filtered_df);  
    let wind_speed = get_wind_speed(&filtered_df);  
    let rainfall = get_rainfall(&filtered_df);  

    WeatherReport {  
        report,  
        min_temp,  
        max_temp,  
        wind_direction,  
        wind_speed,  
        rainfall,  
    }  
}  

fn load_weather_data(file_path: &str) -> Result<DataFrame, Box<dyn Error>> {  
    let df = CsvReader::from_path(file_path)?  
        .infer_schema(None)  
        .has_header(true)  
        .finish()?;  
    Ok(df)  
}  

fn get_city_month_data(  
    mut df: DataFrame,  
    city: &str,  
    year: i32,  
    month: u32,  
) -> DataFrame {  
    let date_series = df.column("Date").unwrap();  
    let dates: Vec<NaiveDate> = date_series  
        .utf8()  
        .unwrap()  
        .into_iter()  
        .map(|opt_s| {  
            opt_s.and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())  
        })  
        .collect();  

    let location_series = df.column("Location").unwrap();  

    let mask: BooleanChunked = dates  
        .iter()  
        .zip(location_series.utf8().unwrap())  
        .map(|(date_opt, loc_opt)| {  
            if let (Some(date), Some(loc)) = (date_opt, loc_opt) {  
                date.year() == year && date.month() == month && loc == city  
            } else {  
                false  
            }  
        })  
        .collect();  

    df.filter(&mask).unwrap_or_else(|_| DataFrame::default())  
}  

fn generate_prompt(df: &DataFrame, city: &str, year: i32, month: u32) -> String {  
    let csv_data = df.to_csv(String::new()).unwrap_or_else(|_| "".to_string());  
    format!(  
        "Below is the weather data for {} during {}/{}:\n\n{}",  
        city, month, year, csv_data  
    )  
}  

fn load_file_content(path: &str) -> String {  
    fs::read_to_string(path).unwrap_or_else(|_| "".to_string())  
}  

fn get_env_api_key() -> String {  
    env::var("API_KEY").unwrap_or_else(|_| "".to_string())  
}  

fn call_openai_api(  
    prompt: &str,  
    system_prompt_path: &str,  
    example_input_path: &str,  
    example_output_path: &str,  
) -> Result<String, Box<dyn Error>> {  
    let rt = Runtime::new()?;  

    let response = rt.block_on(async {  
        let client = Client::new();  
        let api_key = get_env_api_key();  
        let system_prompt = load_file_content(system_prompt_path);  
        let example_input = load_file_content(example_input_path);  
        let example_output = load_file_content(example_output_path);  

        let request_body = serde_json::json!({  
            "model": "llama3-8b-8192",  
            "messages": [  
                {"role": "system", "content": system_prompt},  
                {"role": "user", "content": example_input},  
                {"role": "assistant", "content": example_output},  
                {"role": "user", "content": prompt}  
            ]  
        });  

        let res = client  
            .post("https://api.openai.com/v1/chat/completions")  
            .bearer_auth(api_key)  
            .json(&request_body)  
            .send()  
            .await?;  

        let res_json: serde_json::Value = res.json().await?;  
        Ok(res_json["choices"][0]["message"]["content"]  
            .as_str()  
            .unwrap_or("")  
            .to_string())  
    })?;  

    Ok(response)  
}  

fn get_min_temp(df: &DataFrame) -> Vec<Option<f32>> {  
    df.column("MinTemp")  
        .unwrap()  
        .f32()  
        .unwrap()  
        .into_iter()  
        .collect()  
}  

fn get_max_temp(df: &DataFrame) -> Vec<Option<f32>> {  
    df.column("MaxTemp")  
        .unwrap()  
        .f32()  
        .unwrap()  
        .into_iter()  
        .collect()  
}  

fn get_wind_direction(df: &DataFrame) -> String {  
    df.column("WindGustDir")  
        .unwrap()  
        .utf8()  
        .unwrap()  
        .mode()  
        .get(0)  
        .unwrap_or("")  
        .to_string()  
}  

fn get_wind_speed(df: &DataFrame) -> Vec<Option<f32>> {  
    df.column("WindGustSpeed")  
        .unwrap()  
        .f32()  
        .unwrap()  
        .into_iter()  
        .collect()  
}  

fn get_rainfall(df: &DataFrame) -> Vec<Option<f32>> {  
    df.column("Rainfall")  
        .unwrap()  
        .f32()  
        .unwrap()  
        .into_iter()  
        .collect()  
}