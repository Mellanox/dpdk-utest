use std::io::{Read, Write};
use std::{io, thread, time};
use std::path::Path;
use std::ops::Add;
use std::time::{SystemTime};
use std::fs;
use std::os::linux::fs::MetadataExt;
use ssh2::{Channel, Error, ExtendedData, Session};

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
                    log::warn!("read error {:?}", io_err);
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

fn rsh_session(hostname:&str) -> Session {
    let stream = std::net::TcpStream::connect(format!("{}:22", hostname)).unwrap();
    stream.set_nonblocking(true).unwrap();
    let mut session = Session::new().unwrap();
    let private_key = Path::new("/home/garik/.ssh/id_rsa");
    session.set_blocking(false);
    session.set_tcp_stream(stream);
    ssh2_nb(|| session.handshake());
    ssh2_nb(|| session.userauth_pubkey_file("root", None, &private_key, None));
    session
}

pub fn rsh_send_dir(hostname:&str, local:&Path, remote:&Path) {
    let meta = match fs::metadata(local) {
        Ok(v) => v,
        Err(err) => panic!("invalid local entry {:?} err: {:?}", local, err)
    };
    if !meta.is_dir() { panic!("not a directory: {}", local.to_str().unwrap())}
    let (_, status ) = rsh_exec(hostname,
                                &format!("mkdir -p {}", remote.to_str().unwrap()));
    if status != 0 {
        panic!("failed to create remote directory {hostname}:{}", remote.to_str().unwrap())
    }
    let direntry = fs::read_dir(local).unwrap();
    direntry.for_each(|result| {
        match result {
            Ok(entry) => {
                let basename = entry.file_name();
                let remote_file = remote.join(Path::new(&basename));
                rsh_send_file(hostname, &entry.path(), &remote_file);
            },
            Err(e) => { panic!("failed to read directory {}:{:?}", local.to_str().unwrap(), e)}
        }
    });
}

pub fn rsh_send_file(hostname:&str, local:&Path, remote:&Path) {
    let meta = match fs::metadata(local) {
        Ok(v) => v,
        Err(err) => panic!("invalid local entry {:?} err: {:?}", local, err)
    };
    let session = rsh_session(hostname);
    let mut ch = ssh2_nb(||session.scp_send(remote,
                                              (meta.st_mode() & 0777) as i32, // ???
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
    // I don't understand how scp_send assigns remote mode bits
    rsh_exec(hostname, &format!("chmod 444 {}\n", remote.to_str().unwrap()));
}

pub fn rsh_connect(hostname:&str) -> Channel {
    let session = rsh_session(hostname);
    let mut ch = ssh2_nb(|| session.channel_session()).unwrap();
    ssh2_nb(|| ch.handle_extended_data(ExtendedData::Merge));
    ssh2_nb(|| ch.request_pty("vt100", None, None));
    ch
}

pub fn rsh_command(channel:&mut Channel, rcmd:&str) {
    ssh2_nb(|| channel.exec(rcmd));
}

pub fn rsh_exec(hostname:&str, cmd:&str) -> (String, i32) {
    let mut output = String::new();
    let mut channel = rsh_connect(hostname);
    rsh_command(&mut channel, cmd);
    loop {
        let mut buffer: [u8; 1024] = [0; 1024];
        match channel.read(&mut buffer) {
            Ok(0)  => { break }
            Ok(_) => {
                let aux = String::from_utf8(buffer.into()).unwrap();
                log::trace!(target:"rsh_exec", "{aux}");
                output.push_str(&aux);
            }
            Err(err) =>
                if !continue_after_error(&err) {
                    let os_err = match err.raw_os_error() {
                        Some(e) => e,
                        None => 255 as i32
                    };
                    log::warn!("read error {:?} ({})", err, os_err);
                    break;
                }
        }
    }
    ssh2_nb(|| channel.close());
    let status = channel.exit_status().unwrap();
    (output, status)
}

// fn main() {
//     let mut ch = rsh_connect("pegasus30");
//     nonblocking_ssh(|| ch.exec("/usr/bin/stdbuf -o0 -e0 /bin/bash -i "));
//     println!("{}", rsh_read(&mut ch));
//
//     while !ch.eof() {
//         let mut line = String::new();
//         std::io::stdin().read_line(&mut line).unwrap();
//         rsh_write(&mut ch, line.as_str());
//         println!("{}", rsh_read(&mut ch));
//     }
//     println!("EOF {:?}", ch.exit_status());
// }
