use std::{ffi::CStr, net::Shutdown};

use anyhow::{anyhow, Context, Result};
use libc::{freeifaddrs, getifaddrs, ifaddrs, strncmp};
use socket2::{Domain, Protocol, Socket, Type};

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
        Err(anyhow!("no matching interface found"))
    } else {
        Ok(ifname)
    }
}

fn main() -> Result<()> {
    // get ethernet interface
    let ifname = get_eth_interface().context("could not get ethernet interface")?;
    println!("detected ethernet interface: {}", ifname);

    // set up socket in interface
    let ip_socket = Socket::new(Domain::IPV4, Type::RAW, Protocol::TCP.into())
        .context("failed to create ip socket")?;
    ip_socket
        .bind_device(ifname.as_bytes().into())
        .context("failed to bind ip socket")?;

    // shut down read side since we are only going to write
    ip_socket.shutdown(Shutdown::Read)?;

    Ok(())
}
