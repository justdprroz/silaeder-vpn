use std::fs::OpenOptions;
use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;
use std::sync::Mutex;
use std::io::{prelude::*, Write};
use std::path::Path;

use crate::CONFIG;
use crate::servermanager::CACHE;
use crate::wireguardapi;

type TimeData = HashMap<u64, (u64, u64)>;
type AllData = BTreeMap<u64, TimeData>;

lazy_static::lazy_static! {
    pub static ref MONITORING_DATA: Mutex<AllData> = {
        if !Path::new(&CONFIG.monitoring_data_file).exists() {
            Mutex::new(BTreeMap::new())
        } else {
            let mut buffer: String = String::new();
            let mut file = OpenOptions::new().read(true).open(&CONFIG.monitoring_data_file).unwrap();
            file.read_to_string(&mut buffer).unwrap();
            let converted: AllData = {
                let mut tmp: AllData = BTreeMap::new();
                for timestamp in toml::from_str::<BTreeMap<String, BTreeMap<String, (u64, u64)>>>(&buffer).unwrap() {
                    tmp.insert(timestamp.0.parse::<u64>().unwrap(),{
                        let mut tmp: TimeData = HashMap::new();
                        for user in timestamp.1 {
                            tmp.insert(user.0.parse::<u64>().unwrap(), user.1);
                        }
                        tmp
                    });
                }
                tmp
            };
            Mutex::new(converted)
        }
    };
    pub static ref MONITORING_CACHE: Mutex<TimeData> = {
        if !Path::new(&CONFIG.monitoring_cache_file).exists() {
            Mutex::new(HashMap::new())
        } else {
            let mut buffer: String = String::new();
            let mut file = OpenOptions::new().read(true).open(&CONFIG.monitoring_cache_file).unwrap();
            file.read_to_string(&mut buffer).unwrap();
            let deserialised: TimeData = {
                let mut tmp: TimeData = HashMap::new();
                for peer in toml::from_str::<BTreeMap<String, (u64, u64)>>(&buffer).unwrap() {
                    tmp.insert(peer.0.parse::<u64>().unwrap(), peer.1);
                }
                tmp
            };
            Mutex::new(deserialised)
        }
    };
}

pub fn append_to_file(time: u64, data: TimeData, cache: TimeData) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&CONFIG.monitoring_data_file)
        .unwrap();
    write!(
        file,
        "{}",
        toml::to_string(&BTreeMap::from([(
            time.to_string(),
            {
                let mut tmp: HashMap<String, (u64, u64)> = HashMap::new();
                for user in data {
                    tmp.insert(user.0.to_string(), user.1);
                }
                tmp
            }
        )])).unwrap()).unwrap();
    let mut file = OpenOptions::new()
        .create(true)
        .append(false)
        .truncate(true)
        .write(true)
        .open(&CONFIG.monitoring_cache_file)
        .unwrap();
    write!(
        file,
        "{}",
        toml::to_string(
            &{
                let mut tmp: HashMap<String, (u64, u64)> = HashMap::new();
                for peer in cache {
                    tmp.insert(peer.0.to_string(), peer.1);
                }
                tmp
            }
        ).unwrap()).unwrap();
}

pub fn append_to_map(time: u64, data: TimeData) -> bool {
    if MONITORING_DATA.lock().unwrap().contains_key(&time) {
        false
    } else {
        MONITORING_DATA.lock().unwrap().insert(time, data);
        true
    }
}

pub fn update_usage_data() -> () {
    let raw_data: TimeData = {
        let mut tmp: TimeData = HashMap::new();
        for peer in wireguardapi::get_current_stats() {
            tmp.insert(*CACHE.lock().unwrap().get(&peer.0).unwrap(), peer.1);
        }
        tmp
    };
    let time_stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    for user in MONITORING_CACHE.lock().unwrap().keys() {
        if !raw_data.contains_key(user) {
            MONITORING_CACHE.lock().unwrap().insert(*user, (0, 0));
        }
    }
    let mut data: TimeData = HashMap::new();
    for user in raw_data.keys().cloned() {
        if !MONITORING_CACHE.lock().unwrap().contains_key(&user) {
            MONITORING_CACHE.lock().unwrap().insert(user, (0, 0));
        }
        let last_usage = MONITORING_CACHE.lock().unwrap()[&user].clone();
        if last_usage.0 > raw_data[&user].0 || last_usage.1 > raw_data[&user].1 {
            MONITORING_CACHE.lock().unwrap().insert(user, (0, 0));
        }
        let new_usage = (raw_data[&user].0 - last_usage.0, raw_data[&user].1 - last_usage.1);
        MONITORING_CACHE.lock().unwrap().insert(user, raw_data[&user]);
        data.insert(user, new_usage);
    }
    if append_to_map(time_stamp.clone(), data.clone()) {
        append_to_file(time_stamp.clone(), data.clone(), MONITORING_CACHE.lock().unwrap().clone());
    };
    // serde_json::to_string_pretty(&*MONITORING_DATA.lock().unwrap()).unwrap()
    println!("{:#?}", MONITORING_DATA.lock().unwrap());
}

// pub fn get_usage() -> (HashMap<u64, String>, BTreeMap<String, (u64, u64)>);