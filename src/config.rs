// Import the required dependencies.
use serde_derive::{Serialize, Deserialize};
use std::fs;
use toml;
//use log::{error, info, warn};

// Top level struct to hold the TOML data.
#[derive(Serialize, Deserialize,Clone)]
pub struct ConfigData {
    pub api_config: APIConfig,
    pub cmdn_config: Option<CMDNConfig>,
    pub file_systems_config: Option<FileSystemsConfig>,
    pub kubernetes_config: Option<KubernetesConfig>,
}

// ------------------------------------------------------------------

#[derive(Serialize, Deserialize,Clone)]
pub struct APIConfig {
    pub listen_ip_addr: String,
    pub listen_port: String,
    pub history_depth: usize,
}

impl APIConfig {
    pub fn new(listen_ip_addr: String, listen_port: String, history_depth: usize) -> Self {
        APIConfig {
            listen_ip_addr,
            listen_port,
            history_depth,
        }
    }
}

// ------------------------------------------------------------------

#[derive(Serialize, Deserialize,Clone)]
pub struct CMDNConfig {
    pub get_cpu: bool,
    pub get_mem: bool,
    pub get_root_fs: bool,
	pub get_swap_fs: bool,
	pub get_net: bool,
	pub iface: String,
    pub get_temperature: bool,
    pub temperature_item: String,
    pub polling_secs: usize,
}

impl CMDNConfig {
    pub fn new(
        get_cpu: bool,
        get_mem: bool,
        get_root_fs: bool,
        get_swap_fs: bool,
        get_net: bool,
        iface: String,
        get_temperature: bool,
        temperature_item: String,
        polling_secs: i32,
    ) -> Self {
        CMDNConfig {
            get_cpu,
            get_mem,
            get_root_fs,
            get_swap_fs,
            get_net,
            iface,
            get_temperature,
            temperature_item,
            polling_secs,
        }
    }
}

// ------------------------------------------------------------------


#[derive(Serialize, Deserialize,Clone)]
pub struct FileSystemsConfig {
    pub file_systems: Vec<[String;2]>,
    pub polling_secs: usize,
}

impl FileSystemsConfig {
    pub fn new(file_systems: Vec<[String; 2]>, polling_secs: i32) -> Self {
        FileSystemsConfig {
            file_systems,
            polling_secs,
        }
    }
}

// ------------------------------------------------------------------

#[derive(Serialize, Deserialize,Clone)]
pub struct KubernetesConfig {
    pub master_nodes_ip: Vec<[String;2]>,
    pub worker_nodes_ip: Vec<[String;2]>,
    pub exclude_namespaces: Vec<String>,
    pub polling_secs: usize,
}

impl KubernetesConfig {
    pub fn new(
        master_nodes_ip: Vec<[String; 2]>,
        worker_nodes_ip: Vec<[String; 2]>,
        exclude_namespaces: Vec<String>,
        polling_secs: i32,
    ) -> Self {
        KubernetesConfig {
            master_nodes_ip,
            worker_nodes_ip,
            exclude_namespaces,
            polling_secs,
        }
    }
}

// ------------------------------------------------------------------

#[derive(Serialize, Deserialize,Clone)]
pub struct DescrValuePair {
    pub description: String,
    pub value: String,
}

// ------------------------------------------------------------------

pub fn read_config(toml_filename: &str) -> ConfigData{
    // Read the contents of the file using a `match` block
    // to return the `data: Ok(c)` as a `String`
    // or handle any `errors: Err(_)`.
    let toml_contents:String = match std::fs::read_to_string(toml_filename) {
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

fn write_config(filename: &str,configdata: &ConfigData){
    let toml_string = toml::to_string(configdata)
        .expect("\n[!] Could not encode TOML value")
        .replace("\"", "")
        .replace(" ", "");

    std::fs::write(filename, toml_string)
        .expect("\n[!] Could not write config to file!");
}