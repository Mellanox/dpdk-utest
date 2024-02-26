use std::ops::ControlFlow;
use std::{thread, time};
use std::collections::HashMap;
use std::path::Path;
use serde_yaml::{Mapping, Value};
use crate::{Ops, OpsDb, map_str2val, Tags, CmdLine, InterfaceDB, load_interfaces, trim_hosts_file, map_intefaces, Testpmd, Scapy, store_hosts_interfaces, reset_host};
use crate::rhost::RHost;

pub struct UnitTest {
    pub cmdline:CmdLine,
    pub commands:Mapping,
    pub hosts:Mapping,
    pub tags:Tags,
    pub mt_dev:String,
    pub interfaces:InterfaceDB,
    pub ops_db:OpsDb,
}

impl UnitTest {
    pub fn commands_file(&self) -> &str { self.cmdline.commands.as_str() }
    pub fn hosts_file(&self) -> &str { self.cmdline.hosts.as_str() }
    pub fn show_commands(&self) -> bool { self.cmdline.show }
    pub fn try_reuse_config(&self) -> bool {self.cmdline.reuse_conifiguration }
    pub fn verbose(&self) -> bool { self.cmdline.verbose }

    pub fn fetch_mtdev(&mut self) {

        let rhostname = self.hosts.iter().filter(|(k,_)| {
            (*k).as_str().unwrap().ne("interfaces")
        }).last().unwrap().1.as_mapping().unwrap().get("host").unwrap().as_str().unwrap();

        let mut rhost = RHost::new(rhostname);
        let mst_status = rhost.mst_status();
        assert_ne!(mst_status.len(), 0);
        self.mt_dev = if self.cmdline.mtdev.len() > 0 {
            if mst_status.contains_key(&self.cmdline.mtdev) {
                self.cmdline.mtdev.to_string()
            } else {
                log::info!("invalid MT device {}.", self.cmdline.mtdev);
                std::process::exit(255);
            }
        } else {
            mst_status.keys().last().unwrap().to_string()
        }
    }

    fn reset_test_hosts(&mut self) {
        let mut hosts_db:Vec<String> = vec![];
        let mut threads = vec![];

        for tag in &self.tags {
            if hosts_db.contains(&tag.hostname) { continue }
            hosts_db.push(tag.hostname.clone());

            let mtdev_clone = self.mt_dev.to_string().clone();
            let hostname_clone = tag.hostname.clone();
            threads.push(
                thread::spawn(
                    move ||
                        reset_host(hostname_clone, mtdev_clone)));
        };

        for t in threads {
            t.join().unwrap().unwrap()
        }
    }

    pub fn load_interfaces(&mut self) {
        self.interfaces = if self.try_reuse_config() && self.hosts.contains_key("interfaces") {
            load_interfaces(self.hosts.get("interfaces").unwrap().as_mapping().unwrap())
        } else {
            if self.try_reuse_config() {
                log::info!("ignore fast execution: no \"interfaces\" tag.");
            }
            if self.hosts.contains_key("interfaces") {
                trim_hosts_file(Path::new(self.hosts_file()))
            }
            self.fetch_mtdev();
            self.reset_test_hosts();
            map_intefaces(&self.mt_dev, &self.hosts)
        }
    }

    pub fn init_apps(&mut self) {
        let need_host_config:bool =
            !(self.try_reuse_config() && self.hosts.contains_key("interfaces"));
        for tag in &self.tags {
            let app_cmd = self.commands.get(&tag.tag).unwrap().as_mapping().unwrap();
            let app_config = self.hosts.get(&tag.tag).unwrap().as_mapping().unwrap();
            let hostname = app_config.get("host").unwrap().as_str().unwrap();
            let app_ops: Box<dyn Ops> = match tag.agent.as_str() {
                "testpmd" => {
                    let mut testpmd =
                        Testpmd::new(&tag.tag, hostname, app_cmd, app_config, &mut self.interfaces, need_host_config);
                    testpmd.init();
                    Box::new(testpmd)
                },
                "scapy" => {
                    let (_, netdev_map) = self.interfaces.get(hostname).unwrap();
                    let mut scapy = Scapy::new(&tag.tag, hostname, app_cmd, netdev_map, need_host_config);
                    scapy.init();
                    scapy.init_netdev(netdev_map);
                    Box::new(scapy)
                },
                _ => panic!("unknown agent: \'{}\'", tag.agent)
            };
            self.ops_db.insert(tag.tag.clone(), app_ops);
        }
        if need_host_config {store_hosts_interfaces(&self.interfaces, Path::new(self.hosts_file()));}
    }

    pub fn do_test(&mut self) {
        for elm in self.commands.get("flow").unwrap().as_sequence().unwrap() {
            do_flow(elm.as_mapping().unwrap(), &mut self.ops_db)
        }
    }
}
impl Default for UnitTest {
    fn default() -> Self {
        Self {
            cmdline: Default::default(),
            commands: Default::default(),
            hosts: Default::default(),
            tags:Default::default(),
            mt_dev:Default::default(),
            interfaces:Default::default(),
            ops_db:Default::default(),
        }
    }
}


fn parse_result_map(map:&Mapping) -> (&str, Vec<String>){
    //  Mapping {"and": Sequence [String("Flow rule #0 created"), String("Flow rule #1 created"), String("Flow rule #2 created")]}
    let op = map.keys().last().unwrap().as_str().unwrap();

    let matches = map.get(op).unwrap().as_sequence().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    (op, matches)
}

fn match_and(matches:Vec<String>, ops:&mut Box<dyn Ops>) -> bool {
    let res = matches.iter().try_for_each(|m| {
        return if ops.match_output(m) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(false)
        }
    });
    res == ControlFlow::Continue(())
}

fn match_or(matches:Vec<String>, ops:&mut Box<dyn Ops>) -> bool {
    let res = matches.iter().try_for_each(|m| {
        return if !ops.match_output(m) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(true)
        }
    });
    res == ControlFlow::Break(true)
}

fn match_not(matches:Vec<String>, ops:&mut Box<dyn Ops>) -> bool {
    let res = matches.iter().try_for_each(|m| {
        return if !ops.match_output(m) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(false)
        }
    });
    res == ControlFlow::Continue(())
}

fn do_result(val:&Value, ops:&mut Box<dyn Ops>) {
    ops.read_output();
    let res = match val {
        Value::String(str) => {
            ops.match_output(str)
        },
        Value::Number(x) => {
            ops.match_output(&x.to_string())
        },
        Value::Mapping(map) => {
            let (op, matches) = parse_result_map(map);
            match op {
                "and" => match_and(matches, ops),
                "or" => match_or(matches, ops),
                "not" => match_not(matches, ops),
                _ => panic!("unsupportred match operator: \"{op}\"")
            }
        },
        Value::Null => { true },
        _ => panic!("unsupported result type: {:?}", val)
    };
    if !res { panic!("failed to match {:?}", val)}
}


fn phase_delay() {
    // println!("start: {:?}", time::Instant::now());
    let delay = time::Duration::from_millis(10);
    thread::sleep(delay);
    // println!("stop: {:?}", time::Instant::now());
}

/// "tg": String("sendp(udp_pkt, iface=pf0)")
fn do_flat_command(str:&str, ops:&mut Box<dyn Ops>) {
    ops.do_command(str);
}

/// Mapping {
///    "command": String("start\nset verbose 1\n"),
///    "result": String("Change verbose level from \\d{1,} to 1")
/// },
fn do_pmd_command(map:&Mapping, ops:&mut Box<dyn Ops>) {
    do_flat_command(&map_str2val(map, "command"), ops);
    ops.wait_cmd_completion();
    if let Some(val) = map.get("result") {
        do_result(&val, ops)
    }
}

fn do_phase_result(result:&Mapping, ops_db:&mut OpsDb) {
    for (key, val) in result {
        let tag = key.as_str().unwrap();
        if let Some(ops) = ops_db.get_mut(tag) { do_result(val, ops) }
    }
}

fn do_phase_commands(phase:&Mapping, ops_db:&mut OpsDb) {
    for (key, val) in phase {
        let tag = key.as_str().unwrap();
        if let Some (mut ops) = ops_db.get_mut(tag) {
            match val {
                Value::String(str) => {
                    do_flat_command(str, &mut ops);
                }
                Value::Sequence(seq) => {
                    for val in seq {
                        do_pmd_command(val.as_mapping().unwrap(), &mut ops);
                    }
                }
                Value::Mapping(map) => {
                    do_pmd_command(map, &mut ops);
                },
                _ => panic!("unsupported phase command format: {:#?}", val),
            }
        }
    }
}

fn do_phase(phase:&Mapping, ops_db: &mut OpsDb) {
    for ops in ops_db.values_mut() {
        ops.clear_output();
    }
    do_phase_commands(phase, ops_db);
    phase_delay();
    for ops in ops_db.values_mut() {
        ops.read_output();
    }
    if let Some(val) = phase.get("result") {
        do_phase_result(val.as_mapping().unwrap(), ops_db);
    }
}

pub fn do_flow(map:&Mapping, ops_db: &mut OpsDb) {
    let repeat = match map.get("repeat") {
        Some(val) => val.as_u64().unwrap(),
        None => 1
    };
    for i in 1..=repeat {
        for val in map.get("phases").unwrap().as_sequence().unwrap() {
            let phase = val.as_mapping().unwrap();
            if phase.contains_key("name") {
                log::info!("phase: {} {i}/{repeat}", map_str2val(phase, "name"));
            }
            do_phase(phase, ops_db);
        }
    }
}

fn get_phase_commands(phase:&Mapping, commands:&mut HashMap<&str, String>){
    commands.iter_mut().for_each(|(app, cmds)| {
        phase.iter().for_each(|(key, val)| {
            let tag = key.as_str().unwrap().to_string();
            if !app.contains(&tag) {return}
            match val {
                Value::String(str) => {
                    cmds.push_str(str);
                },
                Value::Sequence(seq) => {
                    seq.iter().for_each(|item| {
                        let map = item.as_mapping().unwrap();
                        cmds.push_str(&map_str2val(map, "command"));
                    })
                },
                _ => ()
            }
        });
    });
}

pub fn show_flow_commands(cmd_map:&Mapping, tags:&Tags) {
    let mut commands:HashMap<&str, String> = HashMap::new();
    let flow = cmd_map.get("flow").unwrap().as_sequence().unwrap();

    tags.iter().for_each(|t| { commands.insert(&t.tag, String::new()); });
    flow.iter().for_each(|val| {
        val.as_mapping().unwrap().get("phases").unwrap()
            .as_sequence().unwrap().iter().for_each(|phase| {
            get_phase_commands(phase.as_mapping().unwrap(), &mut commands);
        })
    });
    commands.iter().for_each(|(tag, cmds)| {
        let cmdline =  match cmd_map.get(tag).unwrap().as_mapping().unwrap()
            .get("cmd") {
            Some(val) => { val.as_str().unwrap() }
            None => "scapy"
        };
        log::info!("# {}> {}", tag, cmdline);
        println!("{cmds}");
    });
}