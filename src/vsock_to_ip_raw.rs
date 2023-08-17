use std::ffi::CStr;

use anyhow::{anyhow, Context, Result};
use libc::{freeifaddrs, getifaddrs, ifaddrs, strncmp};

fn get_eth_interface() -> Result<String> {
    let mut ifap: *mut ifaddrs = std::ptr::null_mut();
    let res = unsafe { getifaddrs(&mut ifap) };

    if res < 0 {
        return Err(anyhow!("failed to query interfaces"));
    }

    let mut ifap_iter = ifap;
    let mut ifname = "".to_owned();
    while !ifap_iter.is_null() {
        let name = unsafe { CStr::from_ptr((*ifap_iter).ifa_name) };
        if unsafe { strncmp(name.as_ptr(), "eth".as_ptr().cast(), 3) } == 0
            || unsafe { strncmp(name.as_ptr(), "wlp".as_ptr().cast(), 3) } == 0
        {
            ifname = name.to_str().context("non utf8 interface")?.to_owned();
            break;
        }
        ifap_iter = unsafe { (*ifap_iter).ifa_next };
    }

    unsafe { freeifaddrs(ifap) };

    if ifname == "" {
        Err(anyhow!("could not find ethernet interface"))
    } else {
        Ok(ifname)
    }
}

fn main() -> Result<()> {
    println!("{}", get_eth_interface()?);

    Ok(())
}
