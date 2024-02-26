use std::collections::HashMap;
use regex::Regex;
use crate::{rsh};

pub const MAX_HOST_PORTS_NUM:usize = 4;
type MstDb = HashMap<String, [String;MAX_HOST_PORTS_NUM]>; // {mt4125, [05:00.0, 05:00.1]}
#[derive(Default)]
struct DevInfo {
    mst_status:MstDb,
}

impl DevInfo {
    fn new() -> Self {
        Default::default()
    }
}

pub struct RHost {
    hostname: String,
    devinfo: DevInfo,
}

impl RHost {
    pub fn new(hostname: &str) -> RHost {
        RHost {
            hostname: hostname.to_string(),
            devinfo: DevInfo::new(),
        }
    }
    pub fn exec(&mut self, cmd:&str) -> (String, i32) {
        rsh::rsh_exec(&self.hostname, &format!("{cmd}\n"))
    }
    pub fn mst_status(&mut self) -> &MstDb {
        const MST_STATUS_REGEX:&str = r#"(?m)/dev/mst/(mt[[:digit:]]{1,}).*([[:xdigit:]]{2}):([[:xdigit:]]{2})\.([[:xdigit:]]{1})"#;

        let re = Regex::new(MST_STATUS_REGEX).unwrap();
        match self.exec(&mut "mst restart". to_string()) {
            (_, 0) => (),
            _ => panic!("{}> failed to reset MST", self.hostname)
        }
        let (output, status) = self.exec(&mut "mst status -v". to_string());
        if status != 0 { panic!("{}> remote command failure: mst status", self.hostname); }
        for (_, [mt_dev, bus, dev,  func]) in
        re.captures_iter(&output).map(|caps| caps.extract()) {
            // println!("mt_dev:{mt_dev} {bus}:{dev}.{func}");

            if !self.devinfo.mst_status.contains_key(mt_dev) {
                self.devinfo.mst_status.insert(mt_dev.to_string(), Default::default());
            }
            let data = self.devinfo.mst_status.get_mut(mt_dev).unwrap();
            let fx = func.parse::<usize>().unwrap();
            data[fx] = format!("0000:{bus}:{dev}.{func}").to_string();
        };
        assert_ne!(self.devinfo.mst_status.len(), 0);
        log::trace!("{} mst status: {:?}", self.hostname, self.devinfo.mst_status);
        &self.devinfo.mst_status
    }
}


