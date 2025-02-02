use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use env_logger::Builder;
use mailwatch::{
    mbsync::MbSyncExecutor,
    timer::run_timer,
    updater::{MailUpdater, MailUpdaterTask},
    watcher::{FileWatcher, FileWatcherError},
};
use serde::Deserialize;
use thiserror::Error;

#[derive(Deserialize, Debug)]
struct DovecotConfig {
    dir: PathBuf,
}

#[derive(Deserialize, Debug)]
struct MbSyncConfig {
    command: String,
    args: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct TimerConfig {
    inboxes: u64,
    all: u64,
}

#[derive(Deserialize, Debug)]
struct Config {
    dovecot: DovecotConfig,
    mbsync: MbSyncConfig,
    timer: TimerConfig,
}

#[derive(Debug, Error)]
enum ConfigError {
    #[error("IO-Error: {0}")]
    IoError(#[from] io::Error),
    #[error("config parse error: {0}")]
    TomlError(#[from] toml::de::Error),
}

fn read_config() -> Result<Config, ConfigError> {
    let config_file = match dirs::config_dir() {
        Some(config_dir) => config_dir.join("mail"),
        None => PathBuf::from(","),
    }
    .join("mailwatch.toml");
    log::info!("try to load {:?}", config_file);
    let mut file = File::open(config_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(toml::from_str(&contents)?)
}

fn queue_filewatch_tasks(
    dir_to_watch: &Path,
    updater: &MailUpdater,
) -> Result<(), FileWatcherError> {
    let file_watcher = FileWatcher::new(dir_to_watch)?;
    while let Ok(event) = file_watcher.wait_for_event(None) {
        updater.queue_task(MailUpdaterTask::new(
            Some(event.account),
            Some(event.mailbox),
        ));
    }
    Ok(())
}

fn get_inboxes(dir: &Path) -> Result<Vec<String>, io::Error> {
    let mut result = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            result.push(
                entry
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
        }
    }
    Ok(result)
}

fn main() {
    Builder::new()
        .filter(None, log::LevelFilter::Info)
        // .filter(Some("localpackage"), log::LevelFilter::Debug)
        .write_style(env_logger::WriteStyle::Auto)
        .init();
    let config = read_config().unwrap();
    //setup executor
    let executor = MbSyncExecutor::new(&config.mbsync.command, &config.mbsync.args);
    //setup updater for task handling
    let updater = MailUpdater::new(move |task| executor.execute(task));
    //setup timer for time based updates
    let timer_updater = updater.clone();
    run_timer(
        config.timer.inboxes,
        config.timer.all,
        get_inboxes(&config.dovecot.dir).unwrap(),
        move |task| {
            timer_updater.queue_task(task);
        },
    );
    //setup filepatcher
    queue_filewatch_tasks(&config.dovecot.dir, &updater).unwrap();
}
