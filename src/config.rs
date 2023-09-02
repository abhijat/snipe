use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use xdg::BaseDirectories;

pub fn get_prefix() -> Result<BaseDirectories> {
    BaseDirectories::with_prefix("snipe").map_err(anyhow::Error::from)
}

pub fn create_data_file_handle(file_name: &str) -> Result<File> {
    let prefix = get_prefix()?;
    let path = prefix.place_data_file(file_name)?;
    File::create(path).map_err(anyhow::Error::from)
}

pub fn get_config_file_path(file_name: &str) -> Result<PathBuf> {
    Ok(get_prefix()?.get_config_file(file_name))
}

pub fn get_data_file_path(file_name: &str) -> Result<PathBuf> {
    Ok(get_prefix()?.get_data_file(file_name))
}

pub fn get_data_file_handle(file_name: &str) -> Result<Option<File>> {
    let path = get_data_file_path(file_name)?;
    if path.exists() {
        return Ok(Some(File::open(path)?));
    } else {
        return Ok(None);
    }
}

pub fn get_config_file_handle(file_name: &str) -> Result<Option<File>> {
    let path = get_config_file_path(file_name)?;
    if path.exists() {
        return Ok(Some(File::open(path)?));
    } else {
        return Ok(None);
    }
}

pub fn create_config_file_handle(file_name: &str) -> Result<File> {
    let prefix = get_prefix()?;
    let path = prefix.place_config_file(file_name)?;
    File::create(path).map_err(anyhow::Error::from)
}

pub trait WritableConfig {
    type Config: DeserializeOwned;

    fn filename() -> String;

    fn config_path() -> Result<PathBuf> {
        get_config_file_path(&Self::filename())
    }

    fn is_config_present() -> Result<bool> {
        Ok(Self::config_path()?.exists())
    }

    fn load() -> Result<Self::Config> {
        let config_file = get_config_file_handle(&Self::filename())?.expect(&format!(
            "A config file should be present at {}",
            Self::filename()
        ));
        let v: Self::Config = serde_json::from_reader(config_file)?;
        Ok(v)
    }
}

fn load_existing_configuration<T>() -> Result<T>
where
    T: WritableConfig + DeserializeOwned,
{
    let f = File::open(T::config_path()?)?;
    let parsed = serde_json::from_reader(f)?;
    Ok(parsed)
}

fn load_default_configuration<T>() -> Result<T>
where
    T: WritableConfig + Default + Serialize,
{
    let config = T::default();

    let mut handle = create_config_file_handle(&T::filename())?;
    handle.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;

    Ok(config)
}

pub fn emplace_default<T>() -> Result<T>
where
    T: WritableConfig + Default + DeserializeOwned + Serialize,
{
    if T::is_config_present()? {
        println!(
            "loading configuration from {}",
            T::config_path()?.to_string_lossy()
        );
        load_existing_configuration::<T>()
    } else {
        println!(
            "storing default configuration in {}",
            T::config_path()?.to_string_lossy()
        );
        load_default_configuration()
    }
}

pub fn load_configuration<T>(custom_path: Option<String>) -> Result<T>
where
    T: Default + DeserializeOwned + WritableConfig + Serialize,
{
    match custom_path {
        None => emplace_default::<T>(),
        Some(p) => {
            let f = File::open(p)?;
            Ok(serde_json::from_reader(f)?)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CommandRunConfig {
    pub command_mappings: HashMap<String, String>,
}

impl WritableConfig for CommandRunConfig {
    type Config = CommandRunConfig;
    fn filename() -> String {
        "command_config.json".to_owned()
    }
}

impl Default for CommandRunConfig {
    fn default() -> Self {
        let mut command_mappings = HashMap::new();
        command_mappings.insert(
            "duck".to_owned(),
            r#"task rp:run-ducktape-tests DUCKTAPE_ARGS="{{test_path}} {{test_args}}""#.to_owned(),
        );
        command_mappings.insert(
            "compile".to_owned(),
            "ninja -C vbuild/{{build_type}}/clang -j 25 bin/{{test_obj}}".to_owned(),
        );
        command_mappings.insert(
            "run".to_owned(), 
            "./tools/cmake_test.py --binary {{pwd}}/vbuild/{{build_type}}/clang/bin/{{test_obj}} {{test_tag_arg}} -- -c1".to_owned()
        );
        Self { command_mappings }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ScanConfig {
    pub cc_test_root: String,
    pub py_test_root: String,
}

impl WritableConfig for ScanConfig {
    type Config = ScanConfig;
    fn filename() -> String {
        "scan_config.json".to_owned()
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            cc_test_root: "src/v".to_owned(),
            py_test_root: "tests/rptest".to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CommandEnv {
    pub envs: HashMap<String, String>,
}

impl WritableConfig for CommandEnv {
    type Config = CommandEnv;
    fn filename() -> String {
        "command_env.json".to_owned()
    }
}

impl Default for CommandEnv {
    fn default() -> Self {
        let mut environment = HashMap::new();
        environment.insert("RP_TRIM_LOGS".to_owned(), "false".to_owned());
        environment.insert("ENABLE_GIT_VERSION".to_owned(), "OFF".to_owned());
        environment.insert("ENABLE_GIT_HASH".to_owned(), "OFF".to_owned());
        environment.insert("REDPANDA_LOG_LEVEL".to_owned(), "trace".to_owned());
        Self { envs: environment }
    }
}
