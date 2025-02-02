use std::{
    io,
    process::{Command, Stdio},
};

use crate::updater::MailUpdaterTask;

pub struct MbSyncExecutor {
    command: String,
    args: Vec<String>,
}

impl MbSyncExecutor {
    pub fn new(command: &String, args: &[String]) -> Self {
        Self {
            command: command.to_owned(),
            args: args.iter().map(|arg| arg.to_owned()).collect(),
        }
    }

    fn execute_command(&self, task: &MailUpdaterTask) -> Result<(), io::Error> {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        match &task.specific_account {
            Some(acc) => {
                let arg = format!(
                    "{}{}",
                    acc,
                    match &task.specific_mailbox {
                        Some(mailbox) => format!(":{}", mailbox),
                        None => "".to_owned(),
                    }
                );
                log::info!("execut command with {}", arg);
                command.arg(arg);
            }
            None => {
                log::info!("execute command with --all");
                command.arg("--all");
            }
        }
        command.spawn()?.wait()?;
        Ok(())
    }

    pub fn execute(&self, task: &MailUpdaterTask) {
        if let Err(err) = self.execute_command(task) {
            log::error!("error while executing command: {}", err);
        }
    }
}
