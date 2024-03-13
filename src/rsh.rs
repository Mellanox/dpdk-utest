use std::io::{Read, Write};
use std::{io, thread, time};
use std::collections::HashMap;
use std::path::Path;
use std::ops::Add;
use std::time::{SystemTime};
use std::fs;
use regex::Regex;
use ssh2::{Channel, Error, ExtendedData, Session};
use crate::{log_target, rsh};
use crate::{RHost};

pub const MAX_HOST_PORTS_NUM:usize = 4;

pub type MlxDev = (String, [String;MAX_HOST_PORTS_NUM]);
pub type MlxDevDb = HashMap<String, [String;MAX_HOST_PORTS_NUM]>; // {mt4125, [05:00.0, 05:00.1]}

fn continue_after_error(err:&io::Error) -> bool {
    match err.kind() {
        io::ErrorKind::WouldBlock => return true,
        _ => return false,
    }
}

const RSH_READ_MAX_DURATION:time::Duration = time::Duration::from_millis(350);
const RSH_READ_IDLE_TIMEOUT:time::Duration = time::Duration::from_millis(10);

fn channel_check_eof(channel: &mut Channel) {
    if channel.eof() {
        panic!("Connection is dead");
    }
}

pub fn rsh_read(channel: &mut Channel) -> String  {
    let mut end = SystemTime::now().add(RSH_READ_MAX_DURATION);
    let mut output = String::new();

    macro_rules! read_idle {
        () => {
                if SystemTime::now().gt(&end) {break}
                thread::sleep(RSH_READ_IDLE_TIMEOUT);
                continue
        };
    }

    loop {
        channel_check_eof(channel);
        let mut buffer: [u8; 1] = [0; 1];
        match channel.read(&mut buffer) {
            Ok(0)  => { read_idle!(); }
            Ok(_) => {
                let aux = String::from_utf8(buffer.into()).unwrap();
                // println!("{aux}");
                output.push_str(&aux);
                end = SystemTime::now().add(RSH_READ_MAX_DURATION);
            }
            Err(err) =>
                if continue_after_error(&err) { read_idle!(); }
                else {panic!("read error {:?}", err)}
        }
    }
    output
}

pub fn rsh_write(ch:&mut Channel, buf:&str) {
    channel_check_eof(ch);
    match ch.write(buf.as_bytes()) {
        Ok(_) => (),
        Err(e) => panic!("write failure: {:?}", e)
    }
}

pub fn ssh2_nb<F, T>(mut func:F) -> Option<T>
where F: FnMut() -> Result<T, Error> {
    loop {
        match func() {
            Ok(val) => {
                return Some(val)
            },
            Err(ssh_err) => {
                let io_err = io::Error::from(ssh_err);
                if !continue_after_error(&io_err) {
                    log::trace!(target: "SSH", "io error {:?}", io_err);
                    return None
                }
            }
        }
    }
}

pub fn is_rhost_alive(hostname:&str) -> bool {
    match std::net::TcpStream::connect(format!("{}:22", hostname)) {
        Ok(stream) => {
            drop(stream);
            true
        }
        Err(_) => false
    }
}

fn rsh_session(rhost:&RHost) -> Session {
    let stream = std::net::TcpStream::connect(format!("{}:22", rhost.hostname)).unwrap();
    stream.set_nonblocking(true).unwrap();
    let mut session = Session::new().unwrap();
    let private_key = Path::new(&rhost.ssh_key);
    session.set_blocking(false);
    session.set_tcp_stream(stream);
    ssh2_nb(|| session.handshake());
    ssh2_nb(|| session.userauth_pubkey_file("root", None, &private_key, None));
    session
}

pub fn rsh_send_dir(rhost:&RHost, local:&Path, remote:&Path) {
    let meta = match fs::metadata(local) {
        Ok(v) => v,
        Err(err) => panic!("invalid local entry {:?} err: {:?}", local, err)
    };
    if !meta.is_dir() { panic!("not a directory: {}", local.to_str().unwrap())}
    let (_, status ) = rsh_exec(rhost,
                                &format!("mkdir -p {}", remote.to_str().unwrap()));
    if status != 0 {
        panic!("failed to create remote directory {}:{}", rhost.hostname, remote.to_str().unwrap())
    }
    let direntry = fs::read_dir(local).unwrap();
    direntry.for_each(|result| {
        match result {
            Ok(entry) => {
                let basename = entry.file_name();
                let remote_file = remote.join(Path::new(&basename));
                rsh_send_file(rhost, &entry.path(), &remote_file);
            },
            Err(e) => { panic!("failed to read directory {}:{:?}", local.to_str().unwrap(), e)}
        }
    });
}

pub fn rsh_send_file(rhost:&RHost, local:&Path, remote:&Path) {
    let meta = match fs::metadata(local) {
        Ok(v) => v,
        Err(err) => panic!("invalid local entry {:?} err: {:?}", local, err)
    };
    let session = rsh_session(rhost);
    let mut ch = ssh2_nb(|| session.scp_send(remote, 0x164 as i32, // 0544
                                             meta.len(), None)).unwrap();
    let read_buffer = std::fs::read(local).unwrap();
    assert!(meta.len() == read_buffer.len() as u64);
    let transfered_size;
    loop {
        // nonblocking_ssh() expects ssh2::Error.
        // Write trait returns std::io::Error.
        match ch.write(read_buffer.as_slice()) {
            Ok(x) => {
                transfered_size = x;
                break;
            },
            Err(err) => {
                match err.kind() {
                    std::io::ErrorKind::WouldBlock => continue,
                    _ => panic!("scp transfer error: {:?}", err.kind())
                }
            }
        }
    }
    assert!(meta.len() == transfered_size as u64);
    ssh2_nb(|| ch.send_eof());
    ssh2_nb(|| ch.wait_eof());
    ssh2_nb(|| ch.wait_close());
    ssh2_nb(|| ch.close());
    ssh2_nb(|| session.disconnect(None, "", None));
}

pub fn rsh_connect(rhost:&RHost) -> Channel {
    let session = rsh_session(rhost);
    let mut ch = ssh2_nb(|| session.channel_session()).unwrap();
    ssh2_nb(|| ch.handle_extended_data(ExtendedData::Merge));
    ssh2_nb(|| ch.request_pty("vt100", None, None));
    ch
}

pub fn rsh_command(channel:&mut Channel, rcmd:&str) {
    ssh2_nb(|| channel.exec(rcmd));
}

pub fn rsh_exec(rhost:&RHost, cmd:&str) -> (String, i32) {
    let mut output = String::new();
    let mut channel = rsh_connect(rhost);
    rsh_command(&mut channel, cmd);
    loop {
        let mut buffer: [u8; 1024] = [0; 1024];
        match channel.read(&mut buffer) {
            Ok(0)  => { break }
            Ok(_) => {
                let aux = String::from_utf8(buffer.into()).unwrap();
                log::trace!(target: &log_target(&rhost.hostname), " rsh exec {aux}");
                output.push_str(&aux);
            }
            Err(err) =>
                if !continue_after_error(&err) {
                    let os_err = match err.raw_os_error() {
                        Some(e) => e,
                        None => 255 as i32
                    };
                    log::trace!(target: &log_target(&rhost.hostname), "rsh read error {:?} ({})", err, os_err);
                    break;
                }
        }
    }
    ssh2_nb(|| channel.close());
    let status = channel.exit_status().unwrap();
    (output, status)
}

pub fn rsh_disconnect(tag:&str, channel:&mut Channel) -> i32 {
    loop {
        let mut buffer: [u8; 1024] = [0; 1024];
        match channel.read(&mut buffer) {
            Ok(0)  => { break }
            Ok(_) => {
                let output = String::from_utf8(buffer.into()).unwrap();
                log::trace!("{output}");
            }
            Err(err) =>
                if !continue_after_error(&err) {
                    let os_err = match err.raw_os_error() {
                        Some(e) => e,
                        None => 255 as i32
                    };
                    log::warn!(target: &log_target(tag), "read error {:?} ({})", err, os_err);
                    return os_err as i32;
                }
        }
    }
    loop {
         if channel.eof() {break}
    }
    ssh2_nb(|| channel.close());
    let status = channel.exit_status().unwrap();
    if status != 0 {
        log::warn!(target: &log_target(tag), "exit status {status}");
        return status
    }
    let sig = channel.exit_signal().unwrap();
    return if sig.exit_signal.is_none() {0} else {
        log::warn!(target: &log_target(tag), "exit signal: {}", sig.exit_signal.unwrap());
        255 as i32
    }
}

pub fn mst_status(rhost:&RHost) -> MlxDevDb {
    const MST_STATUS_REGEX:&str = r#"(?m)/dev/mst/(mt[[:digit:]]{1,}).*([[:xdigit:]]{2}):([[:xdigit:]]{2})\.([[:xdigit:]]{1})"#;

    let mut mst_status = MlxDevDb::new();
    let re = Regex::new(MST_STATUS_REGEX).unwrap();
    match rsh::rsh_exec(rhost, &mut "mst restart". to_string()) {
        (_, 0) => (),
        _ => panic!("{}> failed to reset MST", rhost.hostname)
    }
    let (output, status) = rsh::rsh_exec(rhost, &mut "mst status -v". to_string());
    if status != 0 { panic!("{}> remote command failure: mst status", rhost.hostname); }
    for (_, [mt_dev, bus, dev,  func]) in
    re.captures_iter(&output).map(|caps| caps.extract()) {
        // println!("mt_dev:{mt_dev} {bus}:{dev}.{func}");

        if !mst_status.contains_key(mt_dev) {
            mst_status.insert(mt_dev.to_string(), Default::default());
        }
        let data = mst_status.get_mut(mt_dev).unwrap();
        let fx = func.parse::<usize>().unwrap();
        data[fx] = format!("0000:{bus}:{dev}.{func}").to_string();
    };
    assert_ne!(mst_status.len(), 0);
    log::trace!(target: &log_target(&rhost.hostname), "mst status: {:?}", mst_status);
    mst_status
}