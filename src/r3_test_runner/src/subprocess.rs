use std::{
    ffi::{OsStr, OsString},
    fmt,
    process::{ExitStatus, Stdio},
};
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command},
};

#[derive(Error, Debug)]
pub enum SubprocessError {
    #[error("Could not execute command `{cmd}`")]
    Spawn {
        cmd: Cmd,
        #[source]
        error: std::io::Error,
    },

    #[error("Command `{cmd}` returned {status}")]
    FailStatus { cmd: Cmd, status: ExitStatus },

    #[error("Failed to send input to command `{cmd}`")]
    WriteInput {
        cmd: Cmd,
        #[source]
        error: std::io::Error,
    },
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
            "Executing command `{}` with environment `{}`",
            DisplayCmd(&self.cmd),
            DisplayEnvs(&self.env)
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

    /// Don't reveal the error output unless the command fails.
    pub async fn spawn_expecting_success_quiet(self) -> Result<(), SubprocessError> {
        self.spawn_expecting_success_quiet_with_input(&[]).await
    }

    /// Don't reveal the error output unless the command fails.
    pub async fn spawn_expecting_success_quiet_with_input(
        self,
        stdin_bytes: &[u8],
    ) -> Result<(), SubprocessError> {
        let mut command = self.build_command();
        command.stderr(Stdio::piped());
        command.stdin(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                return Err(SubprocessError::Spawn {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        };

        let stdin = child.stdin.as_mut().unwrap();
        match stdin.write_all(stdin_bytes).await {
            Ok(()) => {}
            Err(e) => {
                return Err(SubprocessError::WriteInput {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        }
        match stdin.shutdown().await {
            Ok(()) => {}
            Err(e) => {
                return Err(SubprocessError::WriteInput {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        };

        let output = match child.wait_with_output().await {
            Ok(output) => output,
            Err(e) => {
                return Err(SubprocessError::Spawn {
                    cmd: self.into_cmd(),
                    error: e,
                })
            }
        };

        if !output.status.success() {
            // Reveal the error output
            use std::io::Write;
            std::io::stderr().write_all(&output.stderr).unwrap();

            return Err(SubprocessError::FailStatus {
                cmd: self.into_cmd(),
                status: output.status,
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
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());

        match command.spawn() {
            Ok(child) => Ok(child),
            Err(e) => Err(SubprocessError::Spawn {
                cmd: self.into_cmd(),
                error: e,
            }),
        }
    }
}

impl fmt::Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        DisplayCmd(&self.0).fmt(f)
    }
}

struct DisplayCmd<'a>(&'a [OsString]);

impl fmt::Display for DisplayCmd<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut it = self.0.iter();
        if let Some(e) = it.next() {
            write!(f, "{}", ShellEscape(e))?;
            for e in it {
                write!(f, " {}", ShellEscape(e))?;
            }
        }
        Ok(())
    }
}

struct DisplayEnvs<'a>(&'a [(OsString, OsString)]);

impl fmt::Display for DisplayEnvs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut it = self.0.iter();
        if let Some(e) = it.next() {
            write!(f, "{}={}", ShellEscape(&e.0), ShellEscape(&e.1))?;
            for e in it {
                write!(f, " {}={}", ShellEscape(&e.0), ShellEscape(&e.1))?;
            }
        }
        Ok(())
    }
}

struct ShellEscape<'a>(&'a std::ffi::OsStr);

impl fmt::Display for ShellEscape<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // These characters need to be quoted or escaped in a bare word
        let special_chars = b"|&;<>()$`\\\"' \t\n\r*?[#~=%";
        // These characters need to be quoted or escaped in double-quotes
        let special_chars_in_dq = b"*?[#~=%";

        let Some(utf8) = self.0.to_str()
        else {
            // Some bytes are unprintable.
            return write!(f, "<unprintable: {:?}>", self.0);
        };

        // All bytes are printable. We might need quoting or escaping some
        // bytes.
        let bytes = utf8.as_bytes();
        if bytes.contains(&b'\'') {
            // Enclose in double quotes
            write!(f, "\"")?;
            let mut utf8 = utf8;
            while !utf8.is_empty() {
                let Some(i) = utf8
                    .as_bytes()
                    .iter()
                    .position(|b| special_chars_in_dq.contains(b))
                else { break };

                write!(f, "{}", &utf8[..i])?;

                // Escape the byte at `i`
                write!(f, "\\{}", utf8.as_bytes()[i] as char)?;

                utf8 = &utf8[i + 1..];
            }
            write!(f, "{utf8}\"")
        } else if bytes.iter().any(|b| special_chars.contains(b)) {
            // Enclose in single quotes
            write!(f, "'{utf8}'")
        } else {
            write!(f, "{utf8}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    #[cfg(unix)]
    fn shell_escape() {
        assert_eq!(ShellEscape(OsStr::new("test")).to_string(), "test");
        assert_eq!(ShellEscape(OsStr::new("te st")).to_string(), "'te st'");
        assert_eq!(
            ShellEscape(OsStr::new("hoge 'piyo'")).to_string(),
            r#""hoge 'piyo'""#
        );
    }
}
