use std::ffi::OsStr;
use std::fs;
use std::process::{Child, Command, Stdio};

use anyhow::{anyhow, Context, Result};

pub struct Cgroups {
    pub free: Vec<String>,
}

impl Cgroups {
    pub fn new() -> Result<Cgroups> {
        Ok(Cgroups {
            free: get_cgroups()?,
        })
    }

    pub fn reserve(&mut self) -> Result<String> {
        if self.free.len() == 0 {
            return Err(anyhow!(""));
        }

        Ok(self.free.swap_remove(0))
    }

    pub fn release(&mut self, cgroup: String) {
        self.free.push(cgroup);
    }

    pub fn execute(
        cgroup: &str,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<Child> {
        let child = Command::new("cgexec")
            .arg("-g")
            .arg("memory,cpu:".to_string() + cgroup)
            .args(args)
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(child)
    }
}

fn get_cgroups() -> Result<Vec<String>> {
    Ok(fs::read_dir("/sys/fs/cgroup")
        .context("Failed to read the directory /sys/fs/cgroup")?
        .filter_map(|dir| {
            dir.ok().and_then(|dir| {
                dir.path().file_name().and_then(|name| {
                    name.to_str().and_then(|x| {
                        if x.starts_with("workerd_") {
                            Some(x.to_owned())
                        } else {
                            None
                        }
                    })
                })
            })
        })
        .collect())
}
