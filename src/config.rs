// Import the required dependencies.
use serde_derive::{Serialize, Deserialize};
use std::fs;
use toml;
use log::{error, info, warn};

// Top level struct to hold the TOML data.
#[derive(Serialize, Deserialize)]
pub struct ConfigData {
    pub config: GeneralConfig,
}
#[derive(Serialize, Deserialize)]
pub struct GeneralConfig {
    pub listen_ip_addr: String,
    pub listen_port: String,
    pub sample_secs: usize,
    pub history_depth: usize,
	pub iface: String,
    pub temp_item: String,
    pub file_systems: Option<Vec<[String;2]>>,
    pub file_systems_sample_secs: Option<usize>,
}

pub fn read_config(toml_filename: &str) -> ConfigData{
    // Read the contents of the file using a `match` block 
    // to return the `data: Ok(c)` as a `String` 
    // or handle any `errors: Err(_)`.
    let toml_contents:String = match fs::read_to_string(toml_filename) {
        // If successful return the files text as `contents`.
        // `c` is a local variable.
        Ok(c) => c,
            
        // Handle the `error` case.
        Err(_) => {
            // Write `msg` to `stderr`.
            eprintln!("[!] Could not read config file `{}`", toml_filename);
            // Exit the program with exit code `1`.
            std::process::exit(1);
        }
    };

    // Use a `match` block to return the 
    // file `contents` as a `Data struct: Ok(d)`
    // or handle any `errors: Err(_)`.
    let config_data: ConfigData = match toml::from_str(&toml_contents) {
        // If successful, return data as `Data` struct.
        // `d` is a local variable.
        Ok(d) => d,
        // Handle the `error` case.
        Err(_) => {
            // Write `msg` to `stderr`.
            eprintln!("[!] Unable to parse config data from `{}`", toml_filename);
            // Exit the program with exit code `1`.
            std::process::exit(1);
        }
    };

    return config_data;
}