pub mod config;

use log::{error, info, warn};

use config::{read_config,ConfigData, APIConfig, CMDNConfig, FileSystemsConfig, KubernetesConfig};

use serde::{Serialize,Deserialize};
use serde_json::json;

use std::thread;
use std::sync::{Arc,Mutex};
use std::time;
use std::fmt;

use sysinfo::{
    Components, Disks, Networks, System,
};

use axum::{
    extract::State,
    //extract::{Path,Extension,State},
    // body::Body,
    // http::StatusCode,
    // response::{IntoResponse, Response},
    // routing::{delete, get, post, put},
    routing::get,
    Router,
    Json,
};

// ------------------------------------------------------------------

const VERSION: &str = "1.0.20240315";

// ------------------------------------------------------------------


#[derive(Serialize,Deserialize,Clone)]
struct Stats {
    basic_stats: BasicStats,
    file_systems_stats:Vec<FileSystemStats>,
    kubernetes_stats:Vec<KubernetesStats>
}

#[derive(Serialize,Deserialize,Clone)]
struct BasicStats {
    cpu: String,
    ram: String,
    root_fs: String,
    swap_fs: String,
    net_down_kbps: String,
    net_up_kbps: String,
    temperature: String,
}

#[derive(Serialize,Deserialize,Clone)]
struct FileSystemStats {
    fs_name:String,
    fs_mount_point: String,   // device in the form of  /dev/disk/by-uuid/2ec8b2d7-7ef5-4b4b-9a03-d19bfe4c76c0
    fs_used_percentage: String,
}

#[derive(Serialize,Deserialize,Clone)]
struct KubernetesStats {
    node_stats:Vec<KubernetesNodeStats>,
}

#[derive(Serialize,Deserialize,Clone)]
struct KubernetesNodeStats {
    node_role: String,
    node_name: String,
    node_ip: String,
    node_basic_stats: BasicStats,
    node_pods: Vec<String>,
    node_pods_max: usize,
}


// ------------------------------------------------------------------


// ------------------------------------------------------------------

// Add whitespace prepending a value
fn add_whitespace (str_to_format: String, chars_tot: u32) -> String{
    // Get the length of the passed string and calculate how many spaces to add
    let char_num = str_to_format.as_bytes().len() as u32;
    let space_num = chars_tot - char_num;

    // Create a new string to add everything to
    let mut ret_string = String::new();

    // Add all the needed spaces to that string
    for _i in 0..space_num { ret_string.push(' '); }

    // Add the original string to it
    ret_string.push_str(&str_to_format);

    ret_string
}

// Get the average core usage
fn get_cpu_use(req_sys: &sysinfo::System) -> f32{
    // Put all of the core loads into a vector
    let mut cpus: Vec<f32> = Vec::new();
    for core in req_sys.cpus() { cpus.push(core.cpu_usage()); }

    // Get the average load
    let cpu_tot: f32 = cpus.iter().sum();
    let cpu_avg: f32 = cpu_tot / cpus.len() as f32;

    cpu_avg
}

// Divide the used RAM by the total RAM
fn get_ram_use(req_sys: &sysinfo::System) -> f32{
    (req_sys.used_memory() as f32) / (req_sys.total_memory() as f32) * 100.
}

// Divide the used swap by the total swap
fn get_swp_use(req_sys: &sysinfo::System) -> f32{
    (req_sys.used_swap() as f32) / (req_sys.total_swap() as f32) * 100.
}

// Divide the available space in  root filesystem by the total space
fn get_root_use(req_disk: &sysinfo::Disks) -> f32{
    let mut ret_value:f32 = 0.0;
    for disk in req_disk.list(){
        if disk.mount_point().to_str().unwrap() == "/" {
            ret_value = ((disk.total_space()-disk.available_space()) as f32) / (disk.total_space() as f32) * 100.;
        }
    }
    return ret_value;
}

// Divide the available space in specified filesystem by the total space
fn get_fs_use(req_disk: &sysinfo::Disks, mount_fs: &str) -> f32{
    let mut ret_value:f32 = 0.0;
    for disk in req_disk.list(){
        if disk.mount_point().to_str().unwrap() == mount_fs {
            ret_value = ((disk.total_space()-disk.available_space()) as f32) / (disk.total_space() as f32) * 100.;
        }
    }
    return ret_value;
}

// Get the total network (down) usage
fn get_tot_ntwk_dwn(req_net: &sysinfo::Networks, polling_secs: &i32) -> i32{
    // Get the total bytes recieved by every network interface
    let mut rcv_tot: Vec<i32> = Vec::new();
    for (_interface_name, ntwk) in req_net.list() {
        rcv_tot.push(ntwk.received() as i32);
    }

    // Total them and convert the bytes to KB
    let ntwk_tot: i32 = rcv_tot.iter().sum();
    let ntwk_processed = (((ntwk_tot*8)/polling_secs) / 1024) as i32;

    ntwk_processed
}

// Get the total network (up) usage
fn get_tot_ntwk_up(req_net: &sysinfo::Networks, polling_secs: &i32) -> i32{
    // Get the total bytes sent by every network interface
    let mut snd_tot: Vec<i32> = Vec::new();
    for (_interface_name, ntwk) in req_net.list() {
        snd_tot.push(ntwk.transmitted() as i32);
    }

    // Total them and convert the bytes to KB
    let ntwk_tot: i32 = snd_tot.iter().sum();
    let ntwk_processed = (((ntwk_tot*8)/polling_secs) / 1024) as i32;

    ntwk_processed
}

// Get the network (down)  usage for an interface
fn get_iface_ntwk_dwn(req_net: &sysinfo::Networks, polling_secs: &i32, iface: &str) -> i32{
    // Get the total bytes recieved by every network interface
    let mut rcv_tot: Vec<i32> = Vec::new();
    for (interface_name, ntwk) in req_net.list() {
        if interface_name == iface {
            //println!("{:?} rx:{} in {} secs --> {} KBps", interface_name,ntwk.received(),polling_secs,((ntwk.received() as i32 /polling_secs) / 1000) as i32 );
            rcv_tot.push(ntwk.received() as i32);
        }
    }

    // Total them and convert the bytes to KB
    let ntwk_tot: i32 = rcv_tot.iter().sum();
    let ntwk_processed = (((ntwk_tot*8)/polling_secs) / 1024) as i32;

    ntwk_processed
}

// Get the network (up) usage for an interface
fn get_iface_ntwk_up(req_net: &sysinfo::Networks, polling_secs: &i32, iface: &str) -> i32{
    // Get the total bytes sent by every network interface
    let mut snd_tot: Vec<i32> = Vec::new();
    for (interface_name, ntwk) in req_net.list() {
        if interface_name == iface {
            //println!("{:?} rx:{} in {} secs --> {} KBps", interface_name,ntwk.transmitted(),polling_secs,((ntwk.transmitted() as i32 /polling_secs) / 1000) as i32 );
            snd_tot.push(ntwk.transmitted() as i32);
        }

    }

    // Total them and convert the bytes to KB
    let ntwk_tot: i32 = snd_tot.iter().sum();
    let ntwk_processed = (((ntwk_tot*8)/polling_secs) / 1024) as i32;

    ntwk_processed
}

// Get the temperature of the CPU
fn get_temp(req_comp: &sysinfo::Components, temp_item: &str) -> i32{
    // For every component, if it's the CPU, put its temperature in variable to return
    let mut wanted_temp: f32 = -1.;
    for comp in req_comp.list() {
        //println!("{:?}", comp.label());
        if comp.label() == temp_item { wanted_temp = comp.temperature();
        }
    }

    wanted_temp as i32
}

// ------------------------------------------------------------------

// API HANDLER: get statistics
async fn api_get_stats(State(stats_data): State<Arc<Mutex<Vec<Stats>>>>,) -> Json<Vec<Stats>> {
    let stats = stats_data.lock().unwrap();
    axum::Json(stats.to_vec())
}

// API HANDLER: Get the temperature items
async fn api_get_temp_items() -> Json<Vec<String>> {
    let current_comp = sysinfo::Components::new_with_refreshed_list();
    let mut ret_vec:Vec<String> = Vec::new();
    for comp in current_comp.list() {
        ret_vec.push(comp.label().to_string());
    }
    axum::Json(ret_vec)
}

// API HANDLER: Get network interfaces
async fn api_get_ntwk_items() -> Json<Vec<String>> {
    let current_net = sysinfo::Networks::new_with_refreshed_list();
    let mut ret_vec:Vec<String> = Vec::new();
    for (interface_name, _ntwk) in current_net.list() {
        ret_vec.push(interface_name.to_string());
    }
    axum::Json(ret_vec)
}

// ------------------------------------------------------------------

fn build_stats( cmdn_polling_secs:i32,
                file_systems_polling_secs:i32,
                kubernetes_polling_secs:i32,
                history_depth:usize,
                iface:String,
                temp_item:String,
                file_systems:Vec<[String;2]>,
                master_nodes_ip:Vec<[String;2]>,
                worker_nodes_ip:Vec<[String;2]>,
                exclude_namespaces:Vec<String>,
                stats_data: Arc<Mutex<Vec<Stats>>>) {


    println!("Building and refreshing stats every {} seconds keeping a history depth of {}",cmdn_polling_secs.to_string(),history_depth.to_string());

    let mut file_systems_refresh_cycles: u64 = 900;
    let mut kubernetes_refresh_cycles: u64 = 900;


    if file_systems_polling_secs > 0 {
        file_systems_refresh_cycles = ((60 as f32/cmdn_polling_secs as f32)*(file_systems_polling_secs as f32/60 as f32)) as u64;
    }
    if kubernetes_polling_secs > 0 {
        kubernetes_refresh_cycles = ((60 as f32/cmdn_polling_secs as f32)*(kubernetes_polling_secs as f32/60 as f32)) as u64;
    }


    let mut loop_count: u64 = 0;

    // Define a system that we will check
    let mut current_sys = sysinfo::System::new_all();
    let mut current_disks = sysinfo::Disks::new_with_refreshed_list();
    let mut current_net = sysinfo::Networks::new_with_refreshed_list();
    let mut current_comp: sysinfo::Components=sysinfo::Components::new();

    let is_file_systems = file_systems.len()>0;
    let mut last_fs_usage : Vec<FileSystemStats> = Vec::new();

    if temp_item.len() >0 {
        current_comp = sysinfo::Components::new_with_refreshed_list();
    }

    loop
    {
        {
            let mut stats = stats_data.lock().unwrap();

            let mut fs_usage : Vec<FileSystemStats> = Vec::new();

            let mut kube_usage : Vec<KubernetesStats> = Vec::new();


            // Refresh the system
            current_sys.refresh_all();
            current_disks.refresh();
            current_net.refresh();

            if temp_item.len() >0 {
                current_comp.refresh();
            }

            // Call each function to get all the values we need
            let cpu_avg = get_cpu_use(&current_sys);
            let ram_prcnt = get_ram_use(&current_sys);
            let root_prcnt = get_root_use(&current_disks);
            let swp_prcnt = get_swp_use(&current_sys);
            let mut temperature = 0;
            if temp_item.len() >0 {
                temperature = get_temp(&current_comp,&temp_item);
            }

            let ntwk_dwn ;
            let ntwk_up ;
            if iface == "total" {
                ntwk_dwn = get_tot_ntwk_dwn(&current_net,&cmdn_polling_secs);
                ntwk_up = get_tot_ntwk_up(&current_net,&cmdn_polling_secs);
            }
            else{
                ntwk_dwn = get_iface_ntwk_dwn(&current_net,&cmdn_polling_secs,&iface);
                ntwk_up = get_iface_ntwk_up(&current_net,&cmdn_polling_secs,&iface);
            }

            if is_file_systems {
                if (loop_count % file_systems_refresh_cycles) == 0 {
                    for fs in file_systems.clone(){
                        let fs_usage_percent= get_fs_use(&current_disks, &fs[1]);
                        fs_usage.push(
                            FileSystemStats{
                                fs_name: fs[0].clone(),
                                fs_mount_point: fs[1].clone(),
                                fs_used_percentage: format! ("{:.1}",fs_usage_percent),
                            }
                        )
                    }
                    last_fs_usage = fs_usage.clone();
                } else {
                    fs_usage = last_fs_usage.clone();
                }
            }

            // if is_kubernetes {
            //     if (loop_count % kubernetes_refresh_cycles) == 0 {
            //         //println!("Refreshing filesystems usagen in cycle {}",loop_count);
            //         for fs in file_systems.clone(){
            //                 KubernetesStats{
            //                     fs_name: fs[0].clone(),
            //                     fs_mount_point: fs[1].clone(),
            //                     fs_used_percentage: format! ("{:.1}",fs_usage_percent),
            //                 }
            //             )
            //         }
            //         last_kube_usage = kube_usage.clone();
            //     } else {
            //         kube_usage = last_kube_usage.clone();
            //     }
            // }

            if stats.len() == history_depth {
                stats.remove(0);
            }

            stats.push(
                Stats{
                    basic_stats: BasicStats{
                        cpu: format! ("{:.1}",cpu_avg),
                        ram: format! ("{:.1}",ram_prcnt),
                        root_fs: format! ("{:.1}",root_prcnt),
                        swap_fs: format! ("{:.1}",swp_prcnt),
                        net_down_kbps: format! ("{:.1}",ntwk_dwn),
                        net_up_kbps: format! ("{:.1}",ntwk_up),
                        temperature: format! ("{:.1}",temperature),
                    },
                    file_systems_stats: fs_usage.clone(),
                    kubernetes_stats: kube_usage.clone(),
                }
            );

            //Print stats vector
            // let mut msg: String;
            // for index in 0..stats.len() {
            //     msg = format!("Index:{} Values: [CPU: {}%|Temp: {}°C|RAM: {}%|ROOT FS: {}%|Download: {}Kbps/{:.1}KiB/s|Upload: {}Kbps/{:.1}KiB/s]", index,stats[index].cpu, stats[index].temperature, stats[index].ram,stats[index].root_fs, stats[index].net_down_kbps,(ntwk_dwn/8.192 as i32), stats[index].net_up_kbps,(ntwk_up/8.192 as i32));
            //     println!("{}", msg);
            // }
            // println!("------------------------------------------------------------------------------------------------");
        }
        // Wait sample_sec seconds
        thread::sleep(time::Duration::from_secs(cmdn_polling_secs.try_into().unwrap()));
        loop_count += 1;
    }
}

// ------------------------------------------------------------------

#[tokio::main]
async fn main() {

    let cmdline: Vec<String> = std::env::args().collect();
    let path_and_prog_name = cmdline[0].as_str();
    let filename_start = path_and_prog_name.rfind('/').unwrap();
    let prog_path = &path_and_prog_name[..(filename_start)];
    let _current_path = String::from(std::env::current_dir().unwrap().to_str().unwrap()) ; //.to_str().unwrap();
    let conf_path_and_file = String::from("config/stats-exporter.conf");

    let config_filename = format!("{}/{}",prog_path,conf_path_and_file);
    let config_data: ConfigData = read_config(config_filename.as_str());

    //API config values
    let listen_ip_addr: String = config_data.api_config.listen_ip_addr;
    let listen_port: String = config_data.api_config.listen_port;
    let history_depth: usize = config_data.api_config.history_depth;

    let API_config: APIConfig = APIConfig::new(
        listen_ip_addr,
        listen_port,
        history_depth);

    let get_cpu: bool;
    let get_mem: bool;
    let get_root_fs: bool;
    let get_swap_fs: bool;
    let get_net: bool;
    let get_temperature: bool;

    let iface: String;
    let iface_clone: String;
    let is_iface_total: bool;
    let temp_item: String;
    let temp_item_clone: String;
    let is_temp_item: bool;
    let cmdn_polling_secs: i32;


    // Cpu, Mem, Disk, Network config values
    if config_data.cmdn_config.is_some(){
        get_cpu = config_data.cmdn_config.clone().unwrap().get_cpu;
        get_mem = config_data.cmdn_config.clone().unwrap().get_mem;
        get_root_fs = config_data.cmdn_config.clone().unwrap().get_root_fs;
        get_swap_fs = config_data.cmdn_config.clone().unwrap().get_swap_fs;
        get_net = config_data.cmdn_config.clone().unwrap().get_net;
        get_temperature = config_data.cmdn_config.clone().unwrap().get_temperature;
        cmdn_polling_secs = config_data.cmdn_config.clone().unwrap().polling_secs.try_into().unwrap();

        iface = config_data.cmdn_config.clone().unwrap().iface;
        iface_clone = iface.clone();
        is_iface_total = iface=="total";
        temp_item = config_data.cmdn_config.clone().unwrap().temperature_item;
        temp_item_clone = temp_item.clone();
        is_temp_item = temp_item.len()>0;
    }
    else {
        get_cpu = false;
        get_mem = false;
        get_root_fs = false;
        get_swap_fs = false;
        get_net = false;
        get_temperature = false;
        cmdn_polling_secs = 0;

        iface = String::from("");
        iface_clone = iface.clone();
        is_iface_total = iface=="total";
        temp_item = String::from("");
        temp_item_clone = temp_item.clone();
        is_temp_item = false;
    }

    let cmdn_config: CMDNConfig = CMDNConfig::new(
        get_cpu,
        get_mem,
        get_root_fs,
        get_swap_fs,
        get_net,
        iface,
        get_temperature,
        temp_item,
        cmdn_polling_secs);


    let file_systems: Vec<[String;2]>;
    let is_file_systems: bool;
    let file_systems_polling_secs:i32;

    // File Systems config values
    if config_data.file_systems_config.is_some(){
        file_systems = config_data.file_systems_config
            .clone()
            .unwrap()
            .file_systems;
        is_file_systems = file_systems.len()>0;
        file_systems_polling_secs = config_data.file_systems_config.clone().unwrap().polling_secs.try_into().unwrap();
    }
    else {
        file_systems = Vec::new();
        is_file_systems = false;
        file_systems_polling_secs = 0;
    }

    let filesystems_config: FileSystemsConfig = FileSystemsConfig::new(
        file_systems,
        file_systems_polling_secs);

    let master_nodes_ip: Vec<[String;2]>;
    let worker_nodes_ip: Vec<[String;2]>;
    let exclude_namespaces: Vec<String>;
    let kubernetes_polling_secs:i32;
    let is_kubernetes: bool;

    // Kubernetes config values
    if config_data.kubernetes_config.is_some(){

        master_nodes_ip = config_data.kubernetes_config
            .clone()
            .unwrap()
            .master_nodes_ip;
        worker_nodes_ip = config_data.kubernetes_config
            .clone()
            .unwrap()
            .worker_nodes_ip;
        exclude_namespaces = config_data.kubernetes_config
            .clone()
            .unwrap()
            .exclude_namespaces;
        kubernetes_polling_secs = config_data.kubernetes_config.clone().unwrap().polling_secs.try_into().unwrap();
        is_kubernetes = master_nodes_ip.len()>0;
    }
    else {
        master_nodes_ip = Vec::new();
        worker_nodes_ip = Vec::new();
        exclude_namespaces = Vec::new();
        kubernetes_polling_secs = 0;
        is_kubernetes = false;
    }

    let kubernetes_config: KubernetesConfig = KubernetesConfig::new(
        master_nodes_ip,
        worker_nodes_ip,
        exclude_namespaces,
        kubernetes_polling_secs);


    println!("------------------------------------------------------------------------");
    println!("           stats-exporter v.{}", VERSION);
    println!("           (c) 2024 - bloque94 ");
    println!("------------------------------------------------------------------------");
    println!("Reading config from:         ´{}´", config_filename);
    println!("------------------------------------------------------------------------");
    println!("  Listen ip address:         ´{}´", listen_ip_addr);
    println!("  Listen port:               ´{}´", listen_port);
    println!("  History depth:             ´{}´", history_depth.to_string());
    println!("------------------------------------------------------------------------");
    println!("  Get CPU stats:             ´{}´", get_cpu);
    println!("  Get MEMORY stats:          ´{}´", get_mem);
    println!("  Get ROOT filesystem stats: ´{}´", get_root_fs);
    println!("  Get SWAP filesystem stats: ´{}´", get_swap_fs);
    println!("  Get NETWORK stats:         ´{}´", get_net);
    println!("  Network Interface:         ´{}´", iface);
    println!("  Get Temperature stats:     ´{}´", get_temperature);
    println!("  Temperature item:          ´{}´", temp_item);
    println!("  Polling seconds:           ´{}´", cmdn_polling_secs.to_string());
    if is_file_systems{
        println!("------------------------------------------------------------------------");
        println!("  File Systems:              ");
        for fs in file_systems{
            println!("                             ´{}´->´{}´",fs[0],fs[1]);
        }
        println!("  File Systems Polling secs: ´{}´", file_systems_polling_secs);
    } else {
        println!("  No filesystem is configured to gather usage stats data");
    }

    if is_kubernetes{
        println!("------------------------------------------------------------------------");
        println!("  Master Nodes:              ");
        for masternodes in master_nodes_ip{
            println!("                             ´{}´->´{}´",masternodes[0],masternodes[1]);
        }
        println!("  Worker Nodes:              ");
        for workernodes  in worker_nodes_ip{
            println!("                             ´{}´->´{}´",workernodes[0],workernodes[1]);
        }
        println!("  Excluded namespaces:              ");
        for ex_namespaces in exclude_namespaces{
            println!("                             ´{}´",ex_namespaces);
        }
        println!("  Kubernetes Polling secs:   ´{}´", kubernetes_polling_secs);
    } else {
        println!("  No kubernetes section is configured to gather usage stats data");
    }

    println!("------------------------------------------------------------------------\n");

    let stats_data: Arc<Mutex<Vec<Stats>>> = Arc::new(Mutex::new(Vec::new()));
    let stats_thread_data: Arc<Mutex<Vec<Stats>>> = Arc::clone(&stats_data);

    std::thread::spawn( move || {
        build_stats(
            cmdn_polling_secs,
            file_systems_polling_secs,
            kubernetes_polling_secs,
            history_depth,
            iface,
            temp_item,
            file_systems,
            master_nodes_ip,
            worker_nodes_ip,
            exclude_namespaces,
            stats_thread_data);
    });

    let api_thread_data:Arc<Mutex<Vec<Stats>>> = Arc::clone(&stats_data);

    let mut help: String = String::from("");
    if is_iface_total {
        if is_temp_item {
            help = format!(
"Hello from getStats! \n\n
Currently building usage statistics for \n    .- cpu,\n    .- memory,\n    .- root filesystem,\n    .- swap,\n
and\n    .-temperature sensor {} \n
and\n    .-total bandwitdth (all interfaces)\n
every {} seconds with a history depth of {} \n\n
Use: \n    /get-stats url to acccess usage statistics\n    /get-ntwk-items url to get the names of the network interfaces available \n    /get-temp-items url to get the list of temperature sensors available",
                temp_item_clone.to_string(),
                cmdn_polling_secs.to_string(),
                history_depth.to_string()
            );
        } else {
            help = format!(
"Hello from getStats! \n\n
Currently building usage statistics for \n    .- cpu,\n    .- memory,\n    .- root filesystem,\n    .- swap,\n
and total bandwitdth (all interfaces)\n
every {} seconds with a history depth of {} \n\n
Use: \n    /get-stats url to acccess usage statistics\n    /get-ntwk-items url to get the names of the network interfaces available \n    /get-temp-items url to get the list of temperature sensors available",
                cmdn_polling_secs.to_string(),
                history_depth.to_string()
            );
        }
    } else {
        if is_temp_item {
            help = format!(
"Hello from getStats! \n\n
Currently building usage statistics for \n    .- cpu,\n    .- memory,\n    .- root filesystem,\n    .- swap,\n
and\n    .-temperature sensor {}\n
and\n    .-bandwitdth on interface {}\n
every {} seconds with a history depth of {} \n\n
Use: \n    /get-stats url to acccess usage statistics\n    /get-ntwk-items url to get the names of the network interfaces available \n    /get-temp-items url to get the list of temperature sensors available",
                temp_item_clone.to_string(),
                iface_clone.to_string(),
                cmdn_polling_secs.to_string(),
                history_depth.to_string()
            );
        } else {
            help = format!(
"Hello from getStats! \n\n
Currently building usage statistics for \n    .- cpu,\n    .- memory,\n    .- root filesystem,\n    .- swap,\n
and\n    .-bandwitdth on interface {}\n
every {} seconds with a history depth of {} \n\n
Use: \n    /get-stats url to acccess usage statistics\n    /get-ntwk-items url to get the names of the network interfaces available \n    /get-temp-items url to get the list of temperature sensors available",
                iface_clone.to_string(),
                cmdn_polling_secs.to_string(),
                history_depth.to_string()
            );
        }
    }

    // API listener
    let app = axum::Router::new()
    .route("/", get( move || async { help }))
    .route("/get-stats", get(api_get_stats))
    .route("/get-temp-items", get(api_get_temp_items))
    .route("/get-ntwk-items", get(api_get_ntwk_items))
    .with_state(api_thread_data);

    println!("API running on http://{}:{}",listen_ip_addr,listen_port);

    // Start Server
    let listener = tokio::net::TcpListener::bind(format!("{}:{}",listen_ip_addr,listen_port)).await.unwrap();
    axum::serve(listener,app).await.unwrap();
    // axum::Server::bind(&format!("{}:{}",listen_ip_addr,listen_port).parse().unwrap())
    //     .serve(app.into_make_service())
    //     .await
    //     .unwrap();

}