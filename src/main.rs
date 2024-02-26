mod input;
mod rsh;
mod rhost;
mod utest;
mod xregex;

use crate::utest::UnitTest;
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
use rhost::RHost;
use crate::rsh::{is_rhost_alive, rsh_exec, rsh_send_dir};

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

trait Ops {
    fn init(&mut self);
    fn core(&mut self) -> &mut Core;
    fn clear_output(&mut self) { self.core().output.clear(); }

    fn do_command(&mut self, cmd:&str) {
        rsh::rsh_write(&mut self.core().channel, format!("{cmd}\n").as_str());
        self.wait_cmd_completion();
    }
    fn wait_cmd_completion(&mut self) {
        let mut miss_count = 0;
        let delim = format!("{}$", self.core().prompt);
        let delay = time::Duration::from_millis(10);
        loop {
            if self.core().channel.eof() {
                panic!("{}> connection is down", self.core().tag);
            }
            self.read_output();
            if self.try_match_output(&delim) {
                break;
            }
            miss_count += 1;
            if (miss_count & 3) != 0 {
                self.do_command("###\n");
                rsh::rsh_write(&mut self.core().channel, "###\n");
            }
            log::trace!("{}> waiting for command completion", self.core().tag);
            thread::sleep(delay); // wait for command to complete
        }
    }
    fn read_output(&mut self) {
        let output = rsh::rsh_read(&mut self.core().channel);
        let delim = self.core().prompt.as_str();
        let delim_ext = format!("{}###", delim);
        if output.len() > 0 {
            let mut filtered = String::new();
            output.split("\r\n")
                .filter(|line| line.len() > 0)
                // .filter(|line| line.ne(&"###"))
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
                log::info!("{}>", self.core().tag);
                println!("{filtered}");
            }
            self.core().output.push_str(&output);
        }
    }

    fn try_match_output(&mut self, expected:&str) -> bool {
        let output= &self.core().output;
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
            log::info!("{}> failed to match \"{}\"", self.core().tag, expected);
            log::trace!("{}: output", self.core().tag);
            log::trace!("{}", self.core().output);
            false
        }
    }
}

/// Vectors do not guaratie stored items order
const APP_CONFIGURATION_COMMANDS_MAX:usize = 8;
type AppConfig = [String;APP_CONFIGURATION_COMMANDS_MAX];

struct Core {
    tag: String,
    hostname: String,
    output: String,
    channel: Channel,
    prompt:String,
}
impl Core {
    fn new(tag:&str, hostname:&str, prompt:&str, ch: Channel) -> Core {
        Core {
            tag: tag.to_string(),
            hostname: hostname.to_string(),
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


    fn configure_host(tag:&str, hostname:&str, commands:&Mapping, interfaces:&InterfaceMap) {
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
                    log::info!("{tag}::{hostname}: {script}");
                    let rcmd = if script.starts_with("shell ") {
                        script[6..].to_string()
                    } else {
                        format!("bash {REMOTE_SCRIPTS_PATH}/{script}\n")
                    };
                    match rsh_exec(hostname,&rcmd) {
                        (_, 0) => (),
                        (_, status) => panic!("{tag}::{hostname}: {script} failed with status {status}")
                    }
                });
            }
        }
    }
}

struct Testpmd {
    core: Core,
}
impl Testpmd {
    fn new(tag:&str, hostname:&str, commands:&Mapping, config:&Mapping,
           interfaces:&mut InterfaceDB, need_host_config:bool) -> Testpmd {
        let mut map = interfaces.get(hostname).unwrap();
        let mut pci_map= &map.0;
        if need_host_config {
            Core::configure_host(tag, hostname, commands, pci_map);
            let pci = Core::translate_interfaces("pci0", pci_map);
            let updated = map_host_interfaces(hostname, &pci);
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
        let mut ch:Channel = rsh::rsh_connect(hostname);
        log::info!("{tag}> {testpmd_cmd}");
        rsh::rsh_command(&mut ch, &mut testpmd_cmd);
        loop {
            if ch.eof() { panic!("{tag}> testpmd failed to initialize"); }
            let output = rsh::rsh_read(&mut ch);
            if output.contains("testpmd>") {break}
            thread::sleep(delay); // wait for testpmd to initialize
        }
        Self {
            core: Core::new(tag, hostname, "testpmd> ", ch),
        }
    }
}

impl Ops for Testpmd {
    fn core(&mut self) -> &mut Core { &mut self.core }

    fn init(&mut self) {
        let cmd = "show port summary all";
        self.do_command(cmd);
        if !self.match_output(r#"^0\s{1,}([0-9A-F]{2}:){5}[0-9A-F]{2}"#) {
            panic!("{}: failed to initiate", self.core().tag);
        }
    }
}

struct Scapy {
    core: Core,
}
impl Scapy {
    fn new(tag:&str, hostname:&str, commands:&Mapping,
           interfaces:&InterfaceMap, need_host_config:bool) -> Scapy {
        if need_host_config {
            Core::configure_host(tag, hostname, commands, interfaces);
        }
        let mut ch:Channel = rsh::rsh_connect(hostname);
        rsh::rsh_command(&mut ch, &mut "python3 -i -u -");
        Self {
            core: Core::new(tag, hostname, ">>> ", ch),
        }
    }

    fn init_netdev(&mut self, netdev_map:&InterfaceMap) {
        let mut ifup_cmd = String::new();
        let mut scapy_cmd = String::new();
        netdev_map.iter().for_each(|(devkey, netdev)| {
            ifup_cmd.push_str(&format!("ip link set up dev {netdev}\n"));
            scapy_cmd.push_str(&format!("{devkey} = \'{netdev}\'\n"));
        });
        let (_, status) = rsh_exec(&self.core.hostname, &ifup_cmd);
        if status != 0 { panic!("{}: failed to set up interfaces", self.core.hostname)}
        self.do_command(&scapy_cmd);
    }
}

impl Ops for Scapy {
    fn core(&mut self) -> &mut Core { &mut self.core }
    fn init(&mut self) {
        let import_cmd = "from scapy.all import *\n";
        let packet = "UDP(sport=1234).show2()";
        self.do_command(import_cmd);
        self.do_command(packet);
        if !self.match_output(r###"sport(\s){1,}= 1234"###) {
            panic!("{}: failed to initiate", self.core().tag);
        }
    }
}


#[derive(Debug)]
struct Tag {
    tag: String,
    agent: String,
    hostname: String,
}

impl Tag {
}

type Tags = Vec<Tag>;

///
/// Build list of application tags
///
fn get_test_tags(commands:&Mapping, hosts:&Mapping) -> Tags {
    let mut tags:Vec<Tag> = commands.iter().filter_map(|(tag, val)| {
        if val.is_mapping() {
            match val.as_mapping().unwrap().get("agent") {
                None => { return None },
                Some(vv) => {
                    let t = tag.as_str().unwrap();
                    if !hosts.contains_key(t) {
                        log::error!("no \'{t}\' tag in hosts file");
                        std::process::exit(255)
                    }
                    let host_map = hosts.get(t).unwrap().as_mapping().unwrap();
                    let hostname = map_str2val(host_map, "host");
                    let tag = Tag {
                        tag: t.to_string(),
                        agent:vv.as_str().unwrap().to_string(),
                        hostname: hostname.to_string(),
                    };
                    Some(tag)
                }
            }
        } else { None }
    }).collect();
    tags.sort_by(|a, b,| {
        let app_cmd_a = commands.get(&a.tag).unwrap().as_mapping().unwrap();
        let app_cmd_b = commands.get(&b.tag).unwrap().as_mapping().unwrap();
        let res_a = app_cmd_a.contains_key("setup");
        let res_b = app_cmd_b.contains_key("setup");
        if res_a == res_b {return Ordering::Equal}
        else if res_a {return Ordering::Less};
        Ordering::Greater
    });
    log::trace!("{:#?}", tags);
    tags
}

type OpsDb = HashMap<String, Box<dyn Ops>>;

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

fn reset_host_configuration(hostname:&str, mtdev:&str) {
    let (_, status) = rsh_exec(hostname, "test -e /workspace");
    log::info!("{hostname}: configuration reset start");
    if status != 0 {  // physical host
        let (_, status) = rsh_exec(hostname, "mst restart");
        if status != 0 { panic!("{hostname}: failed to restart MST"); }
        let (_, status) =
            rsh_exec(hostname,
                     &format!("mlxfwreset -d /dev/mst/{mtdev}_pciconf0 r --yes --level 3"));
        if status != 0 { panic!("{hostname}: failed to reset FW"); }
        let (_, status) = rsh_exec(hostname, "/etc/init.d/openibd force-restart");
        if status != 0 { panic!("{hostname}: failed to restart MLX OFED"); }
    } else { // Linux Cloud host
        log::info!("reboot {hostname} ...");
        let delay = time::Duration::from_millis(500);
        rsh_exec(hostname, "/usr/sbin/reboot");
        loop {
            thread::sleep(delay);
            if is_rhost_alive(hostname) {break}
        }
        log::info!("{hostname} is up");
    }
    let (_, status) = rsh_exec(hostname, "mst restart");
    if status != 0 { panic!("{hostname}: failed to restart MST"); }
    log::info!("{hostname}: configuration reset completed")
}

fn reset_host(hostname:String, mtdev:String) -> std::io::Result<()>{
    reset_host_configuration(&hostname, &mtdev);
    let local_path = Path::new(LOCAL_SCRIPTS_PATH);
    let remote_path = Path::new(REMOTE_SCRIPTS_PATH);
    rsh_send_dir(&hostname, local_path, remote_path);
    Ok(())
}

const DEVLINK_PF_REGEX:&str = r#"(?m)^pci/(\S{12})/.*type eth netdev (\S{1,}) flavour physical port ([0-9]{1,})"#;
const DEVLINK_VFREP_REGEX:&str = r#"(?m)^pci.*netdev (\S{1,}) flavour.*pfnum ([0-9]{1,}) vfnum ([0-9]{1,})"#;

// /sys/bus/pci/devices/0000:05:00.0/virtfn0 -> ../0000:05:00.2
const VF_PCI_REGEX:&str =
    r#"(?m)/sys/bus/pci/devices/[[:xdigit:]]{4}:[[:xdigit:]]{2}:[[:xdigit:]]{2}.([[:xdigit:]]{1})/virtfn([[:digit:]]{1}).*/([[:xdigit:]]{4}:[[:xdigit:]]{2}:[[:xdigit:]]{2}.[[:xdigit:]]{1})"#;

// pci/0000:05:00.2/131072: type eth netdev enp5s0f0v0 flavour virtual splittable false
const VF_NETDEV_REGEX:&str = r#"(?m)^pci/([[:xdigit:]]{4}:[[:xdigit:]]{2}:[[:xdigit:]]{2}.[[:xdigit:]]{1})/.*netdev (\S{1,}) flavour virtual"#;

fn map_host_interfaces(hostname:&str, pci:&str) -> (InterfaceMap, InterfaceMap) {
    let pf_re = Regex::new(DEVLINK_PF_REGEX).unwrap();
    let vfrep_re = Regex::new(DEVLINK_VFREP_REGEX).unwrap();
    let vfpci_re = Regex::new(VF_PCI_REGEX).unwrap();
    let vfnetdev_re = Regex::new(VF_NETDEV_REGEX).unwrap();
    let mut pci_map:InterfaceMap = InterfaceMap::new();
    let mut netdev_map:InterfaceMap = InterfaceMap::new();

    let pci_partial = &pci[..7];
    let (devlink_output, status) =
        rsh_exec(&hostname, &format!("devlink port | grep {pci_partial}\n"));
    if status != 0 { panic!("{hostname}: failed to fetch devlink info") }
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
                rsh_exec(&hostname, &format!("ls -l /sys/bus/pci/devices/{pci_partial}*/virtfn*\n"));
            if status != 0 { panic!("{hostname}: failed to fetch SRIOV info") }

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

fn map_intefaces(mtdev:&str, hosts:&Mapping) -> InterfaceDB {
    let mut imap:InterfaceDB = HashMap::new();
    let mut hosts_db:Vec<String> = vec![];

    hosts.values()
        .filter(|v| (*v).as_mapping().unwrap().contains_key("host"))
        .for_each(|v| {
        let hostname = map_str2val(v.as_mapping().unwrap(), "host");
        if hosts_db.contains(&hostname) { return }
        hosts_db.push(hostname.clone());
        let mut rhost = RHost::new(&hostname);
        let pci = rhost.mst_status().get(mtdev).unwrap()[0].as_str();
        let interfaces = map_host_interfaces(&hostname, pci);
        imap.insert(hostname.clone(), interfaces);
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
            println!("{TERM_GRAPH_BOLD}{}{TERM_GRAPH_RESET}", record.args());
        }
    }

    fn flush(&self) {}
}


fn main() {
    let mut ut:UnitTest = Default::default();
    log::set_logger(&UTEST_LOGGER).unwrap();
    ut.cmdline = CmdLine::new();
    log::set_max_level(
        if ut.verbose() {LevelFilter::Trace}
        else {LevelFilter::Info}
    );
    ut.commands = import_yaml(Path::new(ut.commands_file()));
    ut.hosts = import_yaml(Path::new(ut.hosts_file()));
    ut.tags = get_test_tags(&ut.commands, &ut.hosts);
    if ut.show_commands() {
        utest::show_flow_commands(&ut.commands, &ut.tags);
        return
    }
    ut.load_interfaces();
    ut.init_apps();
    ut.do_test();
    log::info!("PASSED");
}
