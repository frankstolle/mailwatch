use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SendError, Sender},
    thread,
    time::Duration,
};

use notify::{Event, INotifyWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;
use utf7_imap::decode_utf7_imap;

#[derive(Debug, Error)]
pub enum FileWatcherError {
    #[error("notify error: {0}")]
    NotifyError(#[from] notify::Error),
}

#[derive(Debug)]
pub struct FileWatcherEvent {
    pub account: String,
    pub mailbox: String,
}

#[derive(Error, Debug)]
enum ProduceEventError {
    #[error("skip event")]
    Skip,
    #[error("send error: {0}")]
    SendError(#[from] SendError<FileWatcherEvent>),
}

static PATH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^/?([^/]+)/Mail/mailboxes/(.+)/dbox-Mails$").unwrap());

pub struct FileWatcher {
    events: Receiver<FileWatcherEvent>,
    _watcher: INotifyWatcher,
}

impl FileWatcher {
    pub fn new(path: &Path) -> Result<Self, FileWatcherError> {
        let (notify_tx, notify_rx) = mpsc::channel::<Result<Event, notify::Error>>();
        let (events_tx, events_rx) = mpsc::channel::<FileWatcherEvent>();
        let mut watcher = notify::recommended_watcher(notify_tx)?;
        watcher.watch(path, RecursiveMode::Recursive)?;
        let filewatcher = Self {
            events: events_rx,
            _watcher: watcher,
        };
        Self::handle_events(path.to_path_buf(), notify_rx, events_tx);
        Ok(filewatcher)
    }

    pub fn wait_for_event(
        &self,
        timeout: Option<Duration>,
    ) -> Result<FileWatcherEvent, RecvTimeoutError> {
        match timeout {
            Some(timeout) => self.events.recv_timeout(timeout),
            None => self
                .events
                .recv()
                .map_err(|_| RecvTimeoutError::Disconnected),
        }
    }

    fn produce_event(
        events_tx: &Sender<FileWatcherEvent>,
        basepath: &Path,
        path: &Path,
    ) -> Result<(), ProduceEventError> {
        let filename = path
            .file_name()
            .ok_or(ProduceEventError::Skip)?
            .to_str()
            .ok_or(ProduceEventError::Skip)?;
        if filename == "dovecot.index.cache" || filename.starts_with(".temp") {
            return Err(ProduceEventError::Skip);
        }
        let path = if path.is_dir() {
            path
        } else {
            path.parent().ok_or(ProduceEventError::Skip)?
        };
        let path = path
            .as_os_str()
            .to_str()
            .ok_or(ProduceEventError::Skip)?
            .strip_prefix(basepath.to_str().ok_or(ProduceEventError::Skip)?)
            .ok_or(ProduceEventError::Skip)?;
        let caps = PATH_REGEX.captures(path).ok_or(ProduceEventError::Skip)?;
        let account = &caps[1];
        let mailbox = &caps[2];
        events_tx.send(FileWatcherEvent {
            account: account.to_owned(),
            mailbox: decode_utf7_imap(mailbox.to_owned()),
        })?;
        Ok(())
    }

    fn handle_events(
        basepath: PathBuf,
        notify_rx: Receiver<Result<Event, notify::Error>>,
        events_tx: Sender<FileWatcherEvent>,
    ) {
        thread::spawn(move || {
            for res in notify_rx {
                match res {
                    Ok(event) => match event.kind {
                        notify::EventKind::Create(_) => {
                            for path in event.paths {
                                let _ = Self::produce_event(&events_tx, &basepath, &path);
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            for path in event.paths {
                                let _ = Self::produce_event(&events_tx, &basepath, &path);
                            }
                        }
                        notify::EventKind::Modify(_) => {
                            for path in event.paths {
                                let _ = Self::produce_event(&events_tx, &basepath, &path);
                            }
                        }
                        notify::EventKind::Access(_) => {}
                        notify::EventKind::Any => {}
                        notify::EventKind::Other => {}
                    },
                    Err(e) => log::error!("watch error: {:?}", e),
                }
            }
        });
    }
}

#[cfg(test)]
mod test {
    use std::{
        error::Error,
        fs::{self, File},
        path::PathBuf,
        time::Duration,
    };

    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    use crate::watcher::FileWatcher;

    #[fixture]
    fn mail_directory() -> PathBuf {
        let path = TempDir::new().unwrap().into_path();
        fs::create_dir_all(path.join("acc1/Mail/mailboxes/mailbox1/dbox-Mails")).unwrap();
        fs::create_dir_all(path.join("acc1/Mail/mailboxes/mailbox2/dbox-Mails")).unwrap();
        fs::create_dir_all(path.join("acc2/Mail/mailboxes/mailbox1/dbox-Mails")).unwrap();
        path
    }

    #[rstest]
    pub fn it_should_reqport_new_files(mail_directory: PathBuf) -> Result<(), Box<dyn Error>> {
        let watcher = FileWatcher::new(&mail_directory).unwrap();
        File::create_new(mail_directory.join("acc1/Mail/mailboxes/mailbox1/dbox-Mails/1.eml"))?;
        let event = watcher
            .wait_for_event(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!("acc1", event.account);
        assert_eq!("mailbox1", event.mailbox);
        Ok(())
    }
    #[rstest]
    pub fn it_should_reqport_moved_files(mail_directory: PathBuf) -> Result<(), Box<dyn Error>> {
        {
            File::create_new(mail_directory.join("acc1/Mail/mailboxes/mailbox1/dbox-Mails/1.eml"))?;
        }
        let watcher = FileWatcher::new(&mail_directory).unwrap();
        fs::rename(
            mail_directory.join("acc1/Mail/mailboxes/mailbox1/dbox-Mails/1.eml"),
            mail_directory.join("acc1/Mail/mailboxes/mailbox2/dbox-Mails/1.eml"),
        )?;
        let event = watcher
            .wait_for_event(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!("acc1", event.account);
        assert_eq!("mailbox1", event.mailbox);
        let event = watcher
            .wait_for_event(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!("acc1", event.account);
        assert_eq!("mailbox2", event.mailbox);
        Ok(())
    }
    #[rstest]
    pub fn it_should_reqport_removed_files(mail_directory: PathBuf) -> Result<(), Box<dyn Error>> {
        File::create_new(mail_directory.join("acc1/Mail/mailboxes/mailbox1/dbox-Mails/1.eml"))?;
        let watcher = FileWatcher::new(&mail_directory).unwrap();
        fs::remove_file(mail_directory.join("acc1/Mail/mailboxes/mailbox1/dbox-Mails/1.eml"))?;
        let event = watcher
            .wait_for_event(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!("acc1", event.account);
        assert_eq!("mailbox1", event.mailbox);
        Ok(())
    }
    #[rstest]
    pub fn it_should_reqport_new_files_in_encoded_folders(
        mail_directory: PathBuf,
    ) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(mail_directory.join("acc1/Mail/mailboxes/Sp&AOQ-ter/dbox-Mails"))
            .unwrap();
        let watcher = FileWatcher::new(&mail_directory).unwrap();
        File::create_new(mail_directory.join("acc1/Mail/mailboxes/Sp&AOQ-ter/dbox-Mails/1.eml"))?;
        let event = watcher
            .wait_for_event(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!("acc1", event.account);
        assert_eq!("Später", event.mailbox);
        Ok(())
    }
    #[rstest]
    pub fn it_should_reqport_new_files_in_encoded_folders_with_subfolder(
        mail_directory: PathBuf,
    ) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(mail_directory.join("acc1/Mail/mailboxes/Sp&AOQ-ter/Documents/dbox-Mails"))
            .unwrap();
        let watcher = FileWatcher::new(&mail_directory).unwrap();
        File::create_new(mail_directory.join("acc1/Mail/mailboxes/Sp&AOQ-ter/Documents/dbox-Mails/1.eml"))?;
        let event = watcher
            .wait_for_event(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!("acc1", event.account);
        assert_eq!("Später/Documents", event.mailbox);
        Ok(())
    }
}
