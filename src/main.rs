mod input;
mod rsh;
mod utest;
mod xregex;

use crate::utest::do_test;
pub use input::CmdLine;
use log::{Record, Level, Metadata, LevelFilter};
use std::fs::File;
use std::path::Path;
use std::collections::HashMap;
use std::string::ToString;
use std::{thread, time};
use std::cmp::Ordering;
use std::io::{Read, Write};
use regex::Regex;
use ssh2::{Channel};
use serde_yaml::{Mapping};
use crate::rsh::{is_rhost_alive, mst_status, MlxDev, rsh_exec, rsh_send_dir, rsh_disconnect};

fn read_error(tag:&str, ch:&mut Channel, err:&ssh2::Error) {
    let status = rsh_disconnect(tag, ch);
    log::error!(target: &log_target(tag), "{} ({status})", err.message());
    std::process::exit(status);

}

pub fn log_target(tmpl:&str) -> String {
    format!("{tmpl}")
}

pub fn map_str2val(data:&Mapping, key:&str) -> String {
    match data.get(key) {
        Some(v) => {
            v.as_str().unwrap().to_string()
        },
        None => panic!("bad key {key}")
    }
}

fn import_yaml(filename:&Path) -> Mapping {
    let file = File::open(filename).expect(
        &format!("failed to open {}", filename.display() )
    );
    serde_yaml::from_reader(&file).expect(
        &format!("bad YAML format in {}", filename.display())
    )
}

pub type OpsDb<'a> = HashMap<String, Box<dyn Ops + 'a>>;
pub trait Ops {
    fn init(&mut self);

    fn tag(&self) -> &AppTag;
    fn output(&self) -> &String;
    fn prompt(&self) -> &String;
    fn channel(&mut self) -> &mut Channel;

    fn clear_output(&mut self);

    fn mut_output(&mut self) -> &mut String;

    fn do_command(&mut self, cmd:&str) {
        rsh::rsh_write(&mut self.channel(), format!("{cmd}\n").as_str());
        self.wait_cmd_completion(cmd);
    }
    fn wait_cmd_completion(&mut self, cmd:&str) {
        let mut miss_count = 0;
        let delim = format!("{}$", self.prompt());
        let delay = time::Duration::from_millis(10);
        loop {
            if self.channel().eof() {
                panic!("{}> connection is down", self.tag().app);
            }
            let output = self.read_output();
            if output.len() > 0 {
                if self.try_match_output(&delim) {
                    break;
                } else if !output.contains(&delim) {
                    miss_count += 1;
                    if (miss_count & 1) != 0 {
                        self.do_command("###\n");
                        rsh::rsh_write(&mut self.channel(), "###\n");
                    }
                }
            }
            log::trace!(target:&log_target(&self.tag().app), "waiting for command completion\n{cmd}");
            thread::sleep(delay); // wait for command to complete
        }
    }
    fn read_output(&mut self) -> String {
        match rsh::rsh_read(&mut self.channel()) {
            Ok(output) => {
                let delim = self.prompt().as_str();
                let delim_ext = format!("{}###", delim);
                if output.len() > 0 {
                    let mut filtered = String::new();
                    output.split("\r\n")
                          .filter(|line| line.len() > 0)
                          .filter(|line| line.ne(&delim))
                          .filter(|line| {
                              if line.contains(&delim_ext) || line.contains("###") {false}
                              else {true}
                          })
                          .for_each(|line| {
                              filtered.push_str(line);
                              filtered.push_str("\n");
                          });
                    if filtered.len() > 0 {
                        log::info!(target: &log_target(&self.tag().app), "\n{filtered}", );
                    }
                    self.mut_output().push_str(&output);
                }
                return output
            },
            Err(err) => {
                let app = self.tag().app.clone();
                read_error(&app, &mut self.channel(), &err);
                err.to_string().clone()
            }
        }
    }

    fn try_match_output(&mut self, expected:&str) -> bool {
        let output= self.output();
        match xregex::xregex(output, expected) {
            Some(verdict) => { return verdict },
            None => ()
        }
        let mut pat=r#"(?m)"#.to_string();
        pat.push_str(expected);
        let re = Regex::new(&pat).unwrap();
        return match re.find(output) {
            None => { false },
            Some(_) => true
        }
    }

    fn match_output(&mut self, expected:&str) -> bool {
        if self.try_match_output(expected) {
            true
        } else {
            log::info!(target:&log_target(&self.tag().app),"failed to match \"{}\"", expected);
            log::trace!(target:&log_target(&self.tag().app), "output:\n{}", self.output());
            false
        }
    }
}

/// Vectors do not guaratie stored items order
const APP_CONFIGURATION_COMMANDS_MAX:usize = 8;
type AppConfig = [String;APP_CONFIGURATION_COMMANDS_MAX];

struct Core<'a> {
    tag: &'a AppTag<'a>,
    output: String,
    channel: Channel,
    prompt:String,
}
impl<'a> Core<'a> {
    fn new(tag:&'a AppTag<'a>, prompt:&str, ch: Channel) -> Core<'a> {
        Core {
            tag: tag,
            output: String::new(),
            prompt: prompt.to_string(),
            channel: ch,
        }
    }

    fn translate_interfaces(tmpl:&str, interfaces:&InterfaceMap) -> String {
        let delim = r#"\b"#;
        let mut result = tmpl.to_string();
        interfaces.iter().for_each(|(map, device)| {
            let regex = format!("{delim}{map}{delim}");
            let re = Regex::new(&regex).unwrap();
            match re.find(&result) {
                None => (),
                Some (m) => {
                    let tail = result[m.start()..].replace(map, device);
                    result = result[..m.start()].to_string();
                    result.push_str(&tail);
                }
            }
        });
        result
    }


    fn configure_host(tag:&AppTag, commands:&Mapping, interfaces:&InterfaceMap) {
        match commands.get("setup") {
            None => (),
            Some(setup) => {
                let mut ix:usize = 0;
                let mut list:AppConfig = Default::default();
                setup.as_sequence().unwrap().iter().for_each(|v| {
                    let tmpl = v.as_str().unwrap();
                    list[ix] = Core::translate_interfaces(tmpl, interfaces);
                    ix += 1;
                });
                list.iter().filter(|item| (*item).len() > 0 ).for_each(|script| {
                    log::info!(target: &log_target(&tag.app), "{}::{script}", tag.rhost.hostname);
                    let rcmd = if script.starts_with("shell ") {
                        script[6..].to_string()
                    } else {
                        format!("bash {REMOTE_SCRIPTS_PATH}/{script}\n")
                    };
                    match rsh_exec(tag.rhost, &rcmd) {
                        (_, 0) => (),
                        (_, status) => panic!("{}::{}: {script} failed with status {status}", tag.app, tag.rhost.hostname)
                    }
                });
            }
        }
    }
}

struct Testpmd<'a> {
    core: Core<'a>,
}
impl<'a> Testpmd<'a> {
    fn new(tag:&'a AppTag, commands:&Mapping, config:&Mapping,
           interfaces:&mut InterfaceDB, need_host_config:bool) -> Testpmd<'a> {
        let hostname= &tag.rhost.hostname;
        let mut map = interfaces.get(hostname).unwrap();
        let mut pci_map= &map.0;
        if need_host_config {
            Core::configure_host(tag, commands, pci_map);
            let pci = Core::translate_interfaces("pci0", pci_map);
            let updated = map_host_interfaces(tag.rhost, &pci);
            interfaces.remove(hostname);
            interfaces.insert(hostname.to_string(), updated);
            map = interfaces.get(hostname).unwrap();
            pci_map= &map.0;
        }
        let delay = time::Duration::from_millis(50);
        let tmpl = commands.get("cmd").unwrap().as_str().unwrap();
        let mut testpmd_cmd = config.get("path").unwrap().as_str().unwrap().to_string();
        testpmd_cmd.push_str("/");
        testpmd_cmd.push_str(
            &Core::translate_interfaces(tmpl, pci_map));
        let mut ch:Channel = rsh::rsh_connect(tag.rhost);
        log::info!(target: &log_target(&tag.app), "{testpmd_cmd}");
        rsh::rsh_command(&mut ch, &mut testpmd_cmd);
        loop {
            match rsh::rsh_read(&mut ch) {
                Ok(output) => {
                    log::info!(target:&tag.app, "{output}");
                    if output.contains("testpmd>") {break}
                    thread::sleep(delay); // wait for testpmd to initialize
                },
                Err(err) => {
                    read_error(&tag.app, &mut ch, &err)
                }
            }
        }
        Self {
            core: Core::new(tag, "testpmd> ", ch),
        }
    }
}

impl<'a> Ops for Testpmd<'a> {
    fn init(&mut self) {
        let cmd = "show port summary all";
        self.do_command(cmd);
        if !self.match_output(r#"^0\s{1,}([0-9A-F]{2}:){5}[0-9A-F]{2}"#) {
            panic!("{}: failed to initiate", self.tag().app);
        }
    }

    fn tag(&self) -> &AppTag<'a> { &self.core.tag }
    fn prompt(&self) -> &String { &self.core.prompt }
    fn channel(&mut self) -> &mut Channel { &mut self.core.channel }
    fn clear_output(&mut self) { self.core.output.clear(); }
    fn mut_output(&mut self) -> &mut String { &mut self.core.output }
    fn output(&self) -> &String { &self.core.output }

}

struct Scapy<'a> {
    core: Core<'a>,
}
impl<'a> Scapy<'a> {
    fn new(tag:&'a AppTag, commands:&Mapping,
           interfaces:&InterfaceMap, need_host_config:bool) -> Scapy<'a> {
        if need_host_config {
            Core::configure_host(tag, commands, interfaces);
        }
        let mut ch:Channel = rsh::rsh_connect(tag.rhost);
        rsh::rsh_command(&mut ch, &mut "python3 -i -u -");
        Self {
            core: Core::new(tag, ">>> ", ch),
        }
    }

    fn init_netdev(&mut self, netdev_map:&InterfaceMap) {
        let mut ifup_cmd = String::new();
        let mut scapy_cmd = String::new();
        netdev_map.iter().for_each(|(devkey, netdev)| {
            ifup_cmd.push_str(&format!("ip link set up dev {netdev}\n"));
            scapy_cmd.push_str(&format!("{devkey} = \'{netdev}\'\n"));
        });
        let (_, status) = rsh_exec(self.core.tag.rhost, &ifup_cmd);
        if status != 0 { panic!("{}: failed to set up interfaces", &self.core.tag.rhost.hostname)}
        self.do_command(&scapy_cmd);
    }
}

impl<'a> Ops for Scapy<'a> {
    fn init(&mut self) {
        let import_cmd = "from scapy.all import *\n";
        let packet = "UDP(sport=1234).show2()";
        self.do_command(import_cmd);
        self.do_command(packet);
        if !self.match_output(r###"sport(\s){1,}= 1234"###) {
            panic!("{}: failed to initiate", self.tag().app);
        }
    }
    fn tag(&self) -> &AppTag<'a> { &self.core.tag }
    fn prompt(&self) -> &String { &self.core.prompt }
    fn channel(&mut self) -> &mut Channel { &mut self.core.channel }
    fn output(&self) -> &String { &self.core.output }
    fn mut_output(&mut self) -> &mut String { &mut self.core.output }
    fn clear_output(&mut self) { self.core.output.clear(); }
}

struct Shell<'a> {
    core: Core<'a>,
}

impl<'a> Shell<'a> {
    fn new(tag:&'a AppTag) -> Shell<'a> {
        let mut ch:Channel = rsh::rsh_connect(tag.rhost);
        rsh::rsh_command(&mut ch, &mut "bash");
        Self {
            core: Core::new(tag, ">>>", ch),
        }
    }
}

impl<'a> Ops for Shell<'a> {
    fn init(&mut self) {
        self.do_command("export PS1='>>>'");
    }

    fn tag(&self) -> &AppTag<'a> { &self.core.tag }
    fn prompt(&self) -> &String { &self.core.prompt }
    fn channel(&mut self) -> &mut Channel { &mut self.core.channel }
    fn output(&self) -> &String { &self.core.output }
    fn mut_output(&mut self) -> &mut String { &mut self.core.output }
    fn clear_output(&mut self) { self.core.output.clear(); }
}

pub fn init_apps<'a>(tags:&'a Tags<'a>, interfaces:&mut InterfaceDB, inputs:&Inputs) -> OpsDb<'a> {
    let need_host_config = !inputs.try_reuse_config();
    let mut ops_db = OpsDb::new();
    for tag in tags {
        let app_cmd = inputs.commands.get(&tag.app).unwrap().as_mapping().unwrap();
        let app_config = inputs.hosts.get(&tag.app).unwrap().as_mapping().unwrap();
        let hostname = app_config.get("host").unwrap().as_str().unwrap();
        let app_ops: Box<dyn Ops> = match tag.agent.as_str() {
            "testpmd" => {
                let mut testpmd =
                    Testpmd::new(&tag, app_cmd, app_config, interfaces, need_host_config);
                testpmd.init();
                Box::new(testpmd)
            },
            "scapy" => {
                let (_, netdev_map) = interfaces.get(hostname).unwrap();
                let mut scapy = Scapy::new(&tag, app_cmd, netdev_map, need_host_config);
                scapy.init();
                scapy.init_netdev(netdev_map);
                Box::new(scapy)
            },
            "shell" => {
                let mut shell = Shell::new(&tag);
                shell.init();
                Box::new(shell)
            }
            _ => panic!("unknown agent: \'{}\'", tag.agent)
        };
        ops_db.insert(tag.app.clone(), app_ops);
    }
    if need_host_config {store_hosts_interfaces(interfaces, Path::new(&inputs.cmdline.hosts_file));}
    ops_db
}

pub struct RHost {
    pub hostname:String,
    pub ssh_key:String
}

impl RHost {
    pub fn new(h:String, k:String) -> RHost {
        RHost {
            hostname:h, ssh_key:k
        }
    }
}

pub type RHosts = Vec<RHost>;

pub struct AppTag<'a> {
    pub app: String,
    pub agent: String,
    pub rhost:&'a  RHost,
}

type Tags<'a> = Vec<AppTag<'a>>;

fn get_app_tags(commands:&Mapping) -> Vec<String> {
    let mut t:Vec<String> = vec![];
    commands.iter().for_each(|(tag, val)| {
        if val.is_mapping() {
            match val.as_mapping().unwrap().get("agent") {
                None => (),
                Some(_) => {
                   t.push(tag.as_str().unwrap().to_string());
                }
            }
        } else { () }
    });

    t.sort_by(|a, b,| {
        let app_cmd_a = commands.get(&a).unwrap().as_mapping().unwrap();
        let app_cmd_b = commands.get(&b).unwrap().as_mapping().unwrap();
        let res_a = app_cmd_a.contains_key("setup");
        let res_b = app_cmd_b.contains_key("setup");
        if res_a == res_b {return Ordering::Equal}
        else if res_a {return Ordering::Less};
        Ordering::Greater
    });
    t
}

fn get_rhosts(app_tags:&Vec<String>, inputs:&Inputs) ->RHosts {
    let mut rhosts = RHosts::new();
    app_tags.iter().for_each(|t| {
        if !inputs.hosts.contains_key(t) {
            log::error!(target: &log_target(t), "no \'{t}\' tag in hosts file");
            std::process::exit(255)
        }
        let host_map = inputs.hosts.get(t).unwrap().as_mapping().unwrap();
        let hostname = map_str2val(host_map, "host");
        match rhosts.iter().find(|rh| rh.hostname.eq(&hostname)) {
            Some(_) => (),
            None => {
                rhosts.push(RHost::new(hostname.clone(), inputs.cmdline.ssh_key.clone()));
            }
        }
    });
    rhosts
}

///
/// Build list of application tags
///
fn get_test_tags<'a>(app_tags:&Vec<String>, rhosts:&'a Vec<RHost>, inputs:&Inputs) -> Tags<'a> {
    let mut tags:Tags<'a> = Tags::new();

    app_tags.iter().for_each(|t| {
        let host_map = inputs.hosts.get(t).unwrap().as_mapping().unwrap();
        let hostname = map_str2val(host_map, "host");
        let rhost = rhosts.iter().find(|rh| rh.hostname.eq(&hostname)).unwrap();
        let tag = AppTag {
            app: t.to_string(),
            agent: inputs.commands.get(t).unwrap()
                .as_mapping().unwrap()
                .get("agent").unwrap()
                .as_str().unwrap().to_string(),
            rhost:rhost,
        };
        log::trace!(target: &log_target(&tag.app), "agent={} hostname={}", tag.agent, tag.rhost.hostname);
        tags.push(tag);
    });
    tags
}

///
/// Mapping {
//     "TAG": Mapping {
//         "agent": String("testpmd"),
//         "cmd": String("dpdk-testpmd ..."),
//         "setup": Sequence [
//                     String("config-fdb --mtdev mtX --port 0 --vf 4"),
//                     String("config-fdb --mtdev mtX --port 1 --vf 6")
//                     ]
//         },
//     ...
// }

const REMOTE_SCRIPTS_PATH:&str = "/var/run/dpdk-utest";
const LOCAL_SCRIPTS_PATH:&str = "rhost-config";

fn reset_host_configuration(rhost:&RHost, mtdev:&str) {
    let (_, status) = rsh_exec(rhost, "test -e /workspace");
    log::info!(target: &log_target(&rhost.hostname), "configuration reset start");
    if status != 0 {  // physical host
        log::info!(target: &log_target(&rhost.hostname), "MT dev: {}", mtdev);
        let (_, status) = rsh_exec(rhost, "mst restart");
        if status != 0 { panic!("{}: failed to restart MST", rhost.hostname); }
        let (_, status) =
            rsh_exec(rhost,
                     &format!("mlxfwreset -d /dev/mst/{mtdev}_pciconf0 r --yes --level 3"));
        if status != 0 { panic!("{}: failed to reset FW", rhost.hostname); }
        let (_, status) = rsh_exec(rhost, "/etc/init.d/openibd force-restart");
        if status != 0 {
            log::info!(target: &log_target(&rhost.hostname), "failed to restart MLX OFED");
            if false {
                panic!("{}: failed to restart MLX OFED", rhost.hostname);
            }
        }
    } else { // Linux Cloud host
        log::info!(target: &log_target(&rhost.hostname), "reboot ...");
        let delay = time::Duration::from_millis(500);
        rsh_exec(rhost, "/usr/sbin/reboot");
        loop {
            thread::sleep(delay);
            if is_rhost_alive(&rhost.hostname) {break}
        }
        log::info!(target: &log_target(&rhost.hostname), "host is up");
    }
    let (_, status) = rsh_exec(rhost, "mst restart");
    if status != 0 { panic!("{}: failed to restart MST", rhost.hostname); }
    log::info!(target: &log_target(&rhost.hostname), "configuration reset completed")
}

fn reset_host(rhost:RHost, mtdev:String) -> std::io::Result<()>{
    reset_host_configuration(&rhost, &mtdev);
    Ok(())
}

const DEVLINK_PF_REGEX:&str = r#"(?m)^pci/(\S{12})/.*type eth netdev (\S{1,}) flavour physical port ([0-9]{1,})"#;
const DEVLINK_VFREP_REGEX:&str = r#"(?m)^pci.*netdev (\S{1,}) flavour.*pfnum ([0-9]{1,}) vfnum ([0-9]{1,})"#;

// /sys/bus/pci/devices/0000:05:00.0/virtfn0 -> ../0000:05:00.2
const VF_PCI_REGEX:&str =
    r#"(?m)/sys/bus/pci/devices/[[:xdigit:]]{4}:[[:xdigit:]]{2}:[[:xdigit:]]{2}.([[:xdigit:]]{1})/virtfn([[:digit:]]{1}).*/([[:xdigit:]]{4}:[[:xdigit:]]{2}:[[:xdigit:]]{2}.[[:xdigit:]]{1})"#;

// pci/0000:05:00.2/131072: type eth netdev enp5s0f0v0 flavour virtual splittable false
const VF_NETDEV_REGEX:&str = r#"(?m)^pci/([[:xdigit:]]{4}:[[:xdigit:]]{2}:[[:xdigit:]]{2}.[[:xdigit:]]{1})/.*netdev (\S{1,}) flavour virtual"#;

fn map_host_interfaces(rhost:&RHost, pci:&str) -> (InterfaceMap, InterfaceMap) {
    let pf_re = Regex::new(DEVLINK_PF_REGEX).unwrap();
    let vfrep_re = Regex::new(DEVLINK_VFREP_REGEX).unwrap();
    let vfpci_re = Regex::new(VF_PCI_REGEX).unwrap();
    let vfnetdev_re = Regex::new(VF_NETDEV_REGEX).unwrap();
    let mut pci_map:InterfaceMap = InterfaceMap::new();
    let mut netdev_map:InterfaceMap = InterfaceMap::new();

    log::info!(target: &log_target(&rhost.hostname), "map interfaces on PCI {}", pci);
    let pci_partial = &pci[..7];
    let (devlink_output, status) =
        rsh_exec(rhost, &format!("devlink port | grep {pci_partial}\n"));
    if status != 0 {
        panic!("{}: failed to fetch devlink info", rhost.hostname)
    }
    for (_, [pf_pci, pf_netdev, pf_port]) in
    pf_re.captures_iter(&devlink_output).map(|caps| caps.extract()) {
        let pf_name = format!("pci{pf_port}");
        let netdev_name = format!("pf{pf_port}");
        pci_map.insert(pf_name.clone(), pf_pci.to_string());
        netdev_map.insert(netdev_name.clone(), pf_netdev.to_string());
    }
    for (_, [netdev, pfn, vfn]) in vfrep_re.captures_iter(&devlink_output).map(|caps| caps.extract()) {
        let vfrep_map = format!("pf{pfn}rf{vfn}");
        netdev_map.insert(vfrep_map, netdev.to_string());
    }

    match devlink_output.find("pcivf controller") {
        Some(_) => {
            let (vf_output, status) =
                rsh_exec(rhost, &format!("ls -l /sys/bus/pci/devices/{pci_partial}*/virtfn*\n"));
            if status != 0 { panic!("{}: failed to fetch SRIOV info", rhost.hostname) }

            for (_, [pfx, vfx, vfpci1]) in vfpci_re.captures_iter(&vf_output).map(|caps| caps.extract()) {
                let vf_map = format!("pci{pfx}vf{vfx}");
                pci_map.insert(vf_map, vfpci1.to_string());

                for (_, [vfpci2, vfnetdev]) in vfnetdev_re.captures_iter(&devlink_output).map(|caps| caps.extract()) {
                    if vfpci1.ne(vfpci2) { continue }
                    let vf_netdev_map = format!("pf{pfx}vf{vfx}");
                    netdev_map.insert(vf_netdev_map, vfnetdev.to_string());
                }
            }
        },
        None => ()
    }
    (pci_map, netdev_map)
}

fn map_intefaces(rhosts:&RHosts, inputs:&Inputs) -> InterfaceDB {
    let mut imap:InterfaceDB = HashMap::new();

    rhosts.iter().for_each(|rhost|{
        let mlx_dev = fetch_mtdev(rhost, inputs.mt_dev());
        let pci= mlx_dev.1[0].as_str();
        let interfaces = map_host_interfaces(rhost, pci);
        imap.insert(rhost.hostname.clone(), interfaces);

    });
    imap
}

/// <interface_map: {PCI address|netdev}>
type InterfaceMap = HashMap<String, String>;
/// <hostname: (PCI map, netdev map)>
type InterfaceDB = HashMap<String, (InterfaceMap, InterfaceMap)>;
type DevicesMap = HashMap<String, InterfaceMap>;

fn store_hosts_interfaces(interfaces:&InterfaceDB, hosts_file:&Path) {
    let mut devmap:HashMap<&str, Vec<DevicesMap>> = HashMap::new();
    let mut imap = HashMap::new();

    for (hostname, (pci_map, netdev_map)) in interfaces {
        let mut pci:DevicesMap = HashMap::new();
        pci.insert("pci".to_string(), pci_map.clone());
        let mut netdev:DevicesMap = HashMap::new();
        netdev.insert("netdev".to_string(), netdev_map.clone());
        devmap.insert(hostname, vec![pci, netdev]);
    }

    imap.insert("interfaces", devmap);
    let yaml = serde_yaml::to_string(&imap).unwrap();
    let mut f = File::options().append(true).open(hosts_file).unwrap();
    f.write_all(&yaml.as_bytes()).unwrap();
}

///
/// Mapping {
// "10.237.169.201":
//         Sequence [
//             Mapping {
//             "pci":
//                 Mapping {
//                     "pf0": String("0000:08:00.0"),
//                     "pf1": String("0000:08:00.1"),
//                 }
//             },
//             Mapping {
//             "netdev":
//                 Mapping {
//                     "pf0": String("eth2"),
//                     "pf1": String("eth3"),
//                 }
//             }
//         ]
// }
fn load_interfaces(ival:&Mapping) -> InterfaceDB {
    let mut imap:InterfaceDB = HashMap::new();
    for (hostname, seq_val) in ival {
        let mut pci:InterfaceMap = HashMap::new();
        let mut netdev:InterfaceMap = HashMap::new();

        seq_val.as_sequence().unwrap().iter().for_each(|map_val|{
            let map = map_val.as_mapping().unwrap();
            let (hash, data) = if map.contains_key("pci") {
                (&mut pci, map.get("pci").unwrap().as_mapping().unwrap() )
            } else {
                (&mut netdev, map.get("netdev").unwrap().as_mapping().unwrap())
            };
            data.iter().for_each(|(k,v)| {
                hash.insert(k.as_str().unwrap().to_string(), v.as_str().unwrap().to_string());
            })
        });
        imap.insert(hostname.as_str().unwrap().to_string(), (pci, netdev));
    }
    imap
}

fn reset_test_hosts(rhosts:&RHosts, mt_dev:&String) {
    let mut threads = vec![];

    for rhost in rhosts {

        let rhc = RHost::new(rhost.hostname.clone(), rhost.ssh_key.clone());
        let mtdc= mt_dev.clone();
        threads.push(
            thread::spawn(move || reset_host(rhc, mtdc)));
    };

    for t in threads {
        t.join().unwrap().unwrap()
    }
}

pub fn load_test_interfaces(rhosts:&RHosts, inputs:&Inputs) -> InterfaceDB {
    if inputs.try_reuse_config() {
        load_interfaces(inputs.hosts.get("interfaces").unwrap().as_mapping().unwrap())
    } else {
        if inputs.cmdline.reuse_conifiguration {
            log::info!(target: "conifg", "ignore fast execution: no \"interfaces\" tag.");
        }
        if inputs.hosts.contains_key("interfaces") {
            trim_hosts_file(Path::new(&inputs.cmdline.hosts_file))
        }
        if inputs.reset() {
            let mlx_dev = fetch_mtdev(&rhosts[0], inputs.mt_dev());
            reset_test_hosts(rhosts, &mlx_dev.0);
        } else { log::debug!(target: "config", "skip host reset"); }
        log::debug!(target: "config", "copy config scripts");
        let local_path = Path::new(LOCAL_SCRIPTS_PATH);
        let remote_path = Path::new(REMOTE_SCRIPTS_PATH);
        for rhost in rhosts {
            rsh_send_dir(&rhost, local_path, remote_path);
        }
        map_intefaces(rhosts, inputs)
    }
}

pub fn fetch_mtdev(rhost:&RHost, mt_opt:Option<&str>) -> MlxDev {
    let mst_status = mst_status(rhost);
    assert_ne!(mst_status.len(), 0);

    let mt = match mt_opt {
        Some(mlx_dev) => {
            if mst_status.contains_key(mlx_dev) {
                mlx_dev
            } else {
                log::warn!(target: "config", "invalid MT device host:{} md_dev{}.", rhost.hostname, mlx_dev);
                std::process::exit(255);
            }
        }
        None => {
            mst_status.keys().last().unwrap()
        }
    }.to_string();

    let pci = mst_status[&mt].clone();
    log::info!(target: "config", "MT device: {:?}", (&mt, &pci));
    (mt, pci)
}

fn trim_hosts_file(hosts:&Path) {
    let mut f = File::options().read(true).write(true).open(hosts).unwrap();
    let mut buffer = String::new();
    f.read_to_string(&mut buffer).unwrap();
    match buffer.find("interfaces:") {
        Some(offset) => { f.set_len(offset as u64).unwrap() }
        None => ()
    }
}

static UTEST_LOGGER:UnitTestLogger = UnitTestLogger;

struct UnitTestLogger;

const TERM_GRAPH_BOLD:&str = "\x1b[1m";
const TERM_GRAPH_RESET:&str = "\x1b[0m";

impl log::Log for UnitTestLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{TERM_GRAPH_BOLD}{}>{TERM_GRAPH_RESET} {}",
                record.metadata().target(), record.args())
        }
    }

    fn flush(&self) {}
}

#[derive(Default)]
pub struct Inputs {
    pub cmdline: CmdLine,
    pub commands: Mapping,
    pub hosts:Mapping,
}

impl Inputs {
    pub fn try_reuse_config(&self) -> bool {
        self.cmdline.reuse_conifiguration &&  self.hosts.contains_key("interfaces")
    }

    pub fn loopback(&self) -> bool {
        self.cmdline.loopback
    }
    pub fn reset(&self) -> bool { self.cmdline.reset }

    pub fn mt_dev(&self) -> Option<&str> {
        if self.cmdline.mlx_dev.len() > 0 { Some(self.cmdline.mlx_dev.as_str()) }
        else { None }
    }
}

fn main() {
    let mut inputs:Inputs = Default::default();
    inputs.cmdline = CmdLine::new();
    log::set_logger(&UTEST_LOGGER).unwrap();
    log::set_max_level(
        if inputs.cmdline.silent {LevelFilter::Warn}
        else if inputs.cmdline.verbose {LevelFilter::Trace}
        else {LevelFilter::Info}
    );
    log::trace!(target: "SSH key", "\'{}\'", inputs.cmdline.ssh_key);
    inputs.commands = import_yaml(Path::new(&inputs.cmdline.commands_file));
    inputs.hosts = import_yaml(Path::new(&inputs.cmdline.hosts_file));
    let app_tags = get_app_tags(&inputs.commands);
    let rhosts = get_rhosts(&app_tags, &inputs);
    let tags = get_test_tags(&app_tags, &rhosts, &inputs);
    if inputs.cmdline.show_commands {
        utest::show_flow_commands(&inputs.commands, &tags);
        return
    }
    let mut interfaces = load_test_interfaces(&rhosts, &inputs);
    let mut ops_db = init_apps(&tags, &mut interfaces, &inputs);
    do_test(&inputs.commands, &mut ops_db);
    log::info!(target: "PASSED", "{}",inputs.cmdline.commands_file.split('/').last().unwrap());
}
