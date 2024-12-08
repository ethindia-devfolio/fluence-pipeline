// src/lib.rs  

use marine_rs_sdk::marine;  
use marine_rs_sdk::module_manifest;  
use marine_rs_sdk::MountedBinaryResult;  

module_manifest!();  

pub fn main() {}  

use serde::{Deserialize, Serialize};  
use std::env;  
use std::fs;  
use std::path::Path;  

use chrono::{Datelike, NaiveDate};  
use polars::prelude::*;  
use reqwest::Client;  
use std::error::Error;  
use tokio::runtime::Runtime;  
use log::info;  

// Data structures to represent the weather report  
#[marine]  
#[derive(Debug, Serialize, Deserialize)]  
pub struct PredictionReport {  
    min_temp: Vec<f32>,  
    wind_direction: Vec<String>,  
    wind_speed: Vec<f32>,  
    rainfall: Vec<f32>,  
}  

#[marine]  
pub fn generate_prediction_weather_report(city: String) -> PredictionReport {  
    let file_path = "./weatherAUS.csv";  
    let system_prompt_path = "./system_prompt1.txt";  

    // Initialize logging (optional)  
    let _ = env_logger::try_init();  

    info!("Loading data...");  
    let df = match load_weather_data(file_path) {  
        Ok(df) => df,  
        Err(e) => {  
            info!("Error loading weather data: {}", e);  
            return empty_prediction_report();  
        }  
    };  

    // Fill NaN values with zero  
    let df_filled = df.fill_null(FillNullStrategy::Zero).unwrap_or(df);  

    let filtered_df = get_city_month_data(&df_filled, &city);  

    let prompt = generate_prompt(&filtered_df, &city);  

    if prompt.starts_with("No data available") {  
        info!("{}", prompt);  
        return empty_prediction_report();  
    }  

    info!("Calling API...");  
    let report = match call_api(&prompt, system_prompt_path) {  
        Ok(report) => report,  
        Err(e) => {  
            info!("Error in API call: {}", e);  
            "".to_string()  
        }  
    };  

    // Parse the API response  
    let (parsed_temp, parsed_wind_direction, parsed_wind_speed, parsed_rainfall) = parse_input(&report);  

    PredictionReport {  
        min_temp: parsed_temp,  
        wind_direction: parsed_wind_direction,  
        wind_speed: parsed_wind_speed,  
        rainfall: parsed_rainfall,  
    }  
}  

fn empty_prediction_report() -> PredictionReport {  
    PredictionReport {  
        min_temp: vec![0.0; 12],  
        wind_direction: vec!["".to_string(); 12],  
        wind_speed: vec![0.0; 12],  
        rainfall: vec![0.0; 12],  
    }  
}  

fn load_weather_data<P: AsRef<Path>>(file_path: P) -> Result<DataFrame, Box<dyn Error>> {  
    let df = CsvReader::from_path(file_path)?  
        .infer_schema(None)  
        .has_header(true)  
        .finish()?;  
    Ok(df)  
}  

fn get_city_month_data(df: &DataFrame, city: &str) -> DataFrame {  
    // Filter data for the specified city  
    let city_filter = df  
        .column("Location")  
        .unwrap()  
        .utf8()  
        .unwrap()  
        .equal(city)  
        .unwrap();  

    let mut filtered_df = df.filter(&city_filter).unwrap();  

    // Ensure 'Date' column is in datetime format  
    let date_series = filtered_df.column("Date").unwrap();  
    let dates: Vec<Option<NaiveDate>> = date_series  
        .utf8()  
        .unwrap()  
        .into_iter()  
        .map(|opt_s| {  
            opt_s.and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())  
        })  
        .collect();  

    let date_series = Series::new("Date", &dates);  
    filtered_df.replace_or_add(&date_series).unwrap();  

    // Drop rows with invalid or missing dates  
    filtered_df = filtered_df  
        .drop_nulls(None)  
        .unwrap_or_else(|_| DataFrame::default());  

    // Add 'Year-Month' column  
    let year_months: Vec<String> = dates  
        .into_iter()  
        .map(|opt_date| {  
            opt_date  
                .map(|date| format!("{}-{:02}", date.year(), date.month()))  
                .unwrap_or_else(|| "".to_string())  
        })  
        .collect();  

    let year_month_series = Series::new("Year-Month", &year_months);  
    filtered_df  
        .replace_or_add(&year_month_series)  
        .unwrap();  

    // Define aggregation rules  
    let mut groups = vec!["Year-Month"];  
    let mut agg_exprs = vec![];  

    for field in filtered_df.get_schema().iter_fields() {  
        let name = field.name();  
        if name == "Location" || name == "Date" || name == "Year-Month" {  
            continue;  
        } else if matches!(field.data_type(), DataType::Float64 | DataType::Int64 | DataType::Float32 | DataType::Int32) {  
            agg_exprs.push(col(name).mean().alias(name));  
        } else {  
            agg_exprs.push(col(name).mode().alias(name));  
        }  
    }  

    // Group by 'Year-Month' and aggregate  
    let aggregated_df = filtered_df  
        .lazy()  
        .groupby(groups)  
        .agg(agg_exprs)  
        .collect()  
        .unwrap_or_else(|_| DataFrame::default());  

    aggregated_df  
}  

fn generate_prompt(df: &DataFrame, city: &str) -> String {  
    if df.height() == 0 {  
        return format!("No data available for {}.", city);  
    }  

    let mut buffer = Vec::new();  
    let mut csv_writer = CsvWriter::new(&mut buffer);  

    csv_writer  
        .has_headers(true)  
        .with_delimiter(b',')  
        .finish(df)  
        .unwrap();  

    let csv_data = String::from_utf8(buffer).unwrap_or_default();  

    format!(  
        "Below is the weather data for {}:\n\n{}",  
        city, csv_data  
    )  
}  

fn parse_input(report: &str) -> (Vec<f32>, Vec<String>, Vec<f32>, Vec<f32>) {  
    if report.is_empty() {  
        let zero_floats = vec![0.0; 12];  
        let empty_strings = vec!["".to_string(); 12];  
        return (zero_floats.clone(), empty_strings, zero_floats.clone(), zero_floats);  
    }  

    let lines = report.lines();  
    let mut data = std::collections::HashMap::new();  

    for line in lines {  
        if let Some((key, value)) = line.split_once(":") {  
            data.insert(key.trim().to_string(), value.trim().to_string());  
        }  
    }  

    let parse_floats = |s: &str| -> Vec<f32> {  
        s.split(',')  
            .filter_map(|v| v.trim().parse::<f32>().ok())  
            .collect()  
    };  

    let min_temp = data  
        .get("MinTemp")  
        .map(|s| parse_floats(s))  
        .unwrap_or_else(|| vec![0.0; 12]);  

    let wind_direction = data  
        .get("WindDirection")  
        .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())  
        .unwrap_or_else(|| vec!["".to_string(); 12]);  

    let wind_speed = data  
        .get("WindSpeed")  
        .map(|s| parse_floats(s))  
        .unwrap_or_else(|| vec![0.0; 12]);  

    let rainfall = data  
        .get("Rainfall")  
        .map(|s| parse_floats(s))  
        .unwrap_or_else(|| vec![0.0; 12]);  

    (min_temp, wind_direction, wind_speed, rainfall)  
}  

fn load_system_prompt<P: AsRef<Path>>(path: P) -> String {  
    fs::read_to_string(path).unwrap_or_else(|_| "You are a weather analysis expert.".to_string())  
}  

fn get_env_api_key() -> String {  
    env::var("API_KEY").unwrap_or_else(|_| "".to_string())  
}  

fn call_api(prompt: &str, system_prompt_path: &str) -> Result<String, Box<dyn Error>> {  
    let rt = Runtime::new()?;  

    let response = rt.block_on(async {  
        let client = Client::new();  
        let api_key = get_env_api_key();  

        if api_key.is_empty() {  
            return Err("API key not provided".into());  
        }  

        let system_prompt = load_system_prompt(system_prompt_path);  

        let request_body = serde_json::json!({  
            "model": "llama3-8b-8192",  
            "messages": [  
                {"role": "system", "content": system_prompt},  
                {"role": "user", "content": prompt}  
            ]  
        });  

        let res = client  
            .post("https://api.example.com/v1/chat/completions") // Replace with actual API endpoint  
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