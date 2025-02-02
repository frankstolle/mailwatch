use std::{
    thread::{self, sleep},
    time::Duration,
};

use chrono::Utc;

use crate::updater::MailUpdaterTask;

pub fn run_timer<F>(inboxes_secs: u64, all_secs: u64, accounts: Vec<String>, mut callback: F)
where
    F: FnMut(MailUpdaterTask) + Send + 'static,
{
    thread::spawn(move || {
        //trigger a all at beginning
        callback(MailUpdaterTask::new(None, None));
        let now = Utc::now();
        let mut nextrun_all = now + Duration::from_secs(all_secs);
        let mut nextrun_inboxes = now + Duration::from_secs(inboxes_secs);

        loop {
            let now = Utc::now();
            let wait_duration = (nextrun_all - now).min(nextrun_inboxes - now);
            sleep(wait_duration.to_std().unwrap());
            let now = Utc::now();
            if now > nextrun_all {
                log::info!("timer refresh all");
                callback(MailUpdaterTask::new(None, None));
                nextrun_all = now + Duration::from_secs(all_secs);
                nextrun_inboxes = now + Duration::from_secs(inboxes_secs);
            }
            if now > nextrun_inboxes {
                for account in &accounts {
                    log::info!("timer refresh INBOX {}", account);
                    callback(MailUpdaterTask::new(
                        Some(account.to_owned()),
                        Some("INBOX".to_owned()),
                    ))
                }
                nextrun_inboxes = now + Duration::from_secs(inboxes_secs);
            }
        }
    });
}
