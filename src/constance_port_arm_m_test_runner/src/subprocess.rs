use std::{
    ffi::{OsStr, OsString},
    process::{ExitStatus, Stdio},
};
use thiserror::Error;
use tokio::process::{Child, Command};

#[derive(Error, Debug)]
pub enum SubprocessError {
    #[error("Could not execute the command {cmd:?}: {error}")]
    Spawn {
        cmd: Cmd,
        #[source]
        error: std::io::Error,
    },

    #[error("The command {cmd:?} returned {status}")]
    FailStatus { cmd: Cmd, status: ExitStatus },
}

#[derive(Debug)]
pub struct Cmd(Vec<OsString>);

pub struct CmdBuilder {
    cmd: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

impl CmdBuilder {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            cmd: vec![program.as_ref().to_owned()],
            env: vec![],
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.cmd.push(arg.as_ref().to_owned());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd
            .extend(args.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), val.as_ref().to_owned()));
        self
    }

    fn build_command(&self) -> Command {
        log::debug!(
            "Executing the command {:?} with the environment {:?}",
            self.cmd,
            self.env
        );

        let mut cmd = Command::new(&self.cmd[0]);
        cmd.args(self.cmd[1..].iter().cloned());
        cmd.envs(self.env.iter().map(|(k, v)| (k, v)));
        cmd.kill_on_drop(true);
        cmd
    }

    fn into_cmd(self) -> Cmd {
        Cmd(self.cmd)
    }

    pub async fn spawn_expecting_success(self) -> Result<(), SubprocessError> {
        let mut command = self.build_command();

        let status = match command.status().await {
            Ok(status) => status,
            Err(e) => {
                return Err(SubprocessError::Spawn {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        };

        if !status.success() {
            return Err(SubprocessError::FailStatus {
                cmd: self.into_cmd(),
                status,
            });
        }

        Ok(())
    }

    pub async fn spawn_capturing_stdout(self) -> Result<Vec<u8>, SubprocessError> {
        let mut command = self.build_command();
        command.stdout(Stdio::piped());

        let output = match command.output().await {
            Ok(output) => output,
            Err(e) => {
                return Err(SubprocessError::Spawn {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        };

        if !output.status.success() {
            return Err(SubprocessError::FailStatus {
                cmd: self.into_cmd(),
                status: output.status,
            });
        }

        Ok(output.stdout)
    }

    pub fn spawn_and_get_child(self) -> Result<Child, SubprocessError> {
        let mut command = self.build_command();
        command.stdout(Stdio::piped());

        match command.spawn() {
            Ok(child) => Ok(child),
            Err(e) => {
                return Err(SubprocessError::Spawn {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        }
    }
}
