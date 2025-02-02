use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    thread::{self},
};

#[derive(Debug, Clone)]
pub struct MailUpdaterTask {
    pub specific_account: Option<String>,
    pub specific_mailbox: Option<String>,
}

impl MailUpdaterTask {
    pub fn new(specific_account: Option<String>, specific_mailbox: Option<String>) -> Self {
        Self {
            specific_account,
            specific_mailbox,
        }
    }

    pub fn covers(&self, other: &MailUpdaterTask) -> bool {
        let specific_account = match &self.specific_account {
            Some(account) => account,
            None => {
                return true;
            }
        };
        let other_specific_account = match &other.specific_account {
            Some(account) => account,
            None => {
                return false;
            }
        };
        if specific_account != other_specific_account {
            return false;
        }
        let specific_maxilbox = match &self.specific_mailbox {
            Some(mailbox) => mailbox,
            None => {
                return true;
            }
        };
        let other_specific_mailbox = match &other.specific_mailbox {
            Some(mailbox) => mailbox,
            None => return false,
        };
        specific_maxilbox == other_specific_mailbox
    }
}
pub struct MailUpdater {
    queue: Mutex<VecDeque<MailUpdaterTask>>,
    queue_notify: Condvar,
}

impl MailUpdater {
    pub fn new<F>(task_callback: F) -> Arc<Self>
    where
        F: FnMut(&MailUpdaterTask) + Send + 'static,
    {
        let updater = Arc::new(Self {
            queue: Mutex::default(),
            queue_notify: Condvar::new(),
        });
        let thrad_updater = updater.clone();
        thread::spawn(move || {
            thrad_updater.process_queue(task_callback);
        });
        updater
    }

    pub fn process_queue<F>(&self, mut callback: F)
    where
        F: FnMut(&MailUpdaterTask),
    {
        loop {
            let current_task = {
                let mut queue = self.queue.lock().unwrap();
                while queue.is_empty() {
                    queue = self.queue_notify.wait(queue).unwrap();
                }
                queue.front().unwrap().clone()
            };
            callback(&current_task);
            self.queue.lock().unwrap().pop_front();
        }
    }

    pub fn queue_task(&self, task: MailUpdaterTask) {
        let mut queue = self.queue.lock().unwrap();
        if !queue.iter().any(|queued_task| queued_task.covers(&task)) {
            queue.push_back(task);
            self.queue_notify.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MailUpdaterTask;

    #[test]
    fn it_should_cover_tasks() {
        let queued_task = MailUpdaterTask::new(None, None);
        let task = MailUpdaterTask::new(None, None);
        assert!(queued_task.covers(&task));
        let queued_task = MailUpdaterTask::new(Some("account".to_owned()), None);
        let task = MailUpdaterTask::new(None, None);
        assert!(!queued_task.covers(&task));
        let queued_task = MailUpdaterTask::new(Some("account".to_owned()), None);
        let task = MailUpdaterTask::new(Some("account".to_owned()), None);
        assert!(queued_task.covers(&task));
        let queued_task = MailUpdaterTask::new(Some("account1".to_owned()), None);
        let task = MailUpdaterTask::new(Some("account2".to_owned()), None);
        assert!(!queued_task.covers(&task));
        let queued_task = MailUpdaterTask::new(Some("account".to_owned()), None);
        let task = MailUpdaterTask::new(Some("account".to_owned()), Some("mailbox".to_owned()));
        assert!(queued_task.covers(&task));
        let queued_task =
            MailUpdaterTask::new(Some("account".to_owned()), Some("mailbox1".to_owned()));
        let task = MailUpdaterTask::new(Some("account".to_owned()), Some("mailbox2".to_owned()));
        assert!(!queued_task.covers(&task));
        let queued_task =
            MailUpdaterTask::new(Some("account".to_owned()), Some("mailbox1".to_owned()));
        let task = MailUpdaterTask::new(Some("account".to_owned()), None);
        assert!(!queued_task.covers(&task));
        let queued_task =
            MailUpdaterTask::new(Some("account".to_owned()), Some("mailbox1".to_owned()));
        let task = MailUpdaterTask::new(Some("account".to_owned()), Some("mailbox1".to_owned()));
        assert!(queued_task.covers(&task));
    }
}
