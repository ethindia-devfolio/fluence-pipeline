// src/lib.rs  

use marine_rs_sdk::marine;  
use marine_rs_sdk::module_manifest;  

module_manifest!();  

pub fn main() {}  

use polars::prelude::*;  
use serde::{Deserialize, Serialize};  
use std::error::Error;  
use std::fs::File;  

#[marine]  
#[derive(Debug, Serialize, Deserialize)]  
pub struct EnvironmentalReport {  
    city: String,  
    avg_pm25: Option<f64>,  
    avg_pm10: Option<f64>,  
    avg_solar_radiation: Option<f64>,  
    avg_co2_emissions: Option<f64>,  
    error: String,  
}  

#[marine]  
pub fn generate_environmental_report(city: String) -> EnvironmentalReport {  
    let file_path = "./environmental_data.csv";  

    let df = match load_environmental_data(file_path) {  
        Ok(df) => df,  
        Err(e) => {  
            return EnvironmentalReport {  
                city,  
                avg_pm25: None,  
                avg_pm10: None,  
                avg_solar_radiation: None,  
                avg_co2_emissions: None,  
                error: format!("Error loading environmental data: {}", e),  
            }  
        }  
    };  

    let filtered_data = get_city_data(&df, &city);  

    if filtered_data.height() == 0 {  
        return EnvironmentalReport {  
            city,  
            avg_pm25: None,  
            avg_pm10: None,  
            avg_solar_radiation: None,  
            avg_co2_emissions: None,  
            error: format!("No data available for {}.", city),  
        };  
    }  

    let avg_pm25 = calculate_avg(&filtered_data, "PM2.5");  
    let avg_pm10 = calculate_avg(&filtered_data, "PM10");  
    let avg_solar_radiation = calculate_avg(&filtered_data, "Solar_Radiation");  
    let avg_co2_emissions = calculate_avg(&filtered_data, "CO2_Emissions");  

    EnvironmentalReport {  
        city,  
        avg_pm25,  
        avg_pm10,  
        avg_solar_radiation,  
        avg_co2_emissions,  
        error: "".to_string(),  
    }  
}  

fn load_environmental_data(file_path: &str) -> Result<DataFrame, Box<dyn Error>> {  
    let df = CsvReader::from_path(file_path)?  
        .infer_schema(None)  
        .has_header(true)  
        .finish()?;  
    Ok(df)  
}  

fn get_city_data(df: &DataFrame, city: &str) -> DataFrame {  
    let city_column = df.column("City").unwrap();  

    let mask = city_column  
        .utf8()  
        .unwrap()  
        .equal(city)  
        .unwrap();  

    df.filter(&mask).unwrap_or_else(|_| DataFrame::default())  
}  

fn calculate_avg(df: &DataFrame, column_name: &str) -> Option<f64> {  
    df.column(column_name)  
        .ok()  
        .and_then(|col| col.f64().ok())  
        .and_then(|series| series.mean())  
}