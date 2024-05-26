use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::process::{Child, Command, Stdio};

use anyhow::{anyhow, Context, Result};

// Struct to keep track of the free 'cgroups' available to execute code
pub struct Cgroups {
    pub free: HashSet<String>,
}

impl Cgroups {
    pub fn new() -> Result<Cgroups> {
        Ok(Cgroups {
            free: get_cgroups()?,
        })
    }

    // Reserve a 'cgroup' and remove it from the free list
    pub fn reserve(&mut self) -> Result<String> {
        if self.free.is_empty() {
            return Err(anyhow!(""));
        }

        let Some(cgroup) = self.free.drain().next() else {
            return Err(anyhow!(""));
        };

        Ok(cgroup)
    }

    // Release a 'cgroup' and add it back to the free list
    pub fn release(&mut self, cgroup: String) {
        self.free.insert(cgroup);
    }

    // Execute the user code using workerd config in the given 'cgroup' which'll provide memory and cpu for the purpose
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

// Retrieve the names of the 'cgroups' generated inside the enclave to host user code for execution by workerd runtime
fn get_cgroups() -> Result<HashSet<String>> {
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
