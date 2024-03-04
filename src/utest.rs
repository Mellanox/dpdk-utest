use std::ops::ControlFlow;
use std::{thread, time};
use std::collections::HashMap;
use serde_yaml::{Mapping, Value};
use crate::{Ops, OpsDb, map_str2val, Tags};

pub fn do_test(commands:&Mapping, ops_db:&mut OpsDb) {
    for elm in commands.get("flow").unwrap().as_sequence().unwrap() {
        do_flow(elm.as_mapping().unwrap(), ops_db)
    }
}

fn parse_result_map(map:&Mapping) -> (&str, Vec<String>){
    //  Mapping {"and": Sequence [String("Flow rule #0 created"), String("Flow rule #1 created"), String("Flow rule #2 created")]}
    let op = map.keys().last().unwrap().as_str().unwrap();

    let matches = map.get(op).unwrap().as_sequence().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    (op, matches)
}

fn match_and<'a>(matches:Vec<String>, ops:&mut Box<dyn Ops + 'a>) -> bool {
    let res = matches.iter().try_for_each(|m| {
        return if ops.match_output(m) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(false)
        }
    });
    res == ControlFlow::Continue(())
}

fn match_or<'a>(matches:Vec<String>, ops:&mut Box<dyn Ops + 'a>) -> bool {
    let res = matches.iter().try_for_each(|m| {
        return if !ops.match_output(m) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(true)
        }
    });
    res == ControlFlow::Break(true)
}

fn match_not<'a>(matches:Vec<String>, ops:&mut Box<dyn Ops + 'a>) -> bool {
    let res = matches.iter().try_for_each(|m| {
        return if !ops.match_output(m) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(false)
        }
    });
    res == ControlFlow::Continue(())
}

fn do_result<'a>(val:&Value, ops:&mut Box<dyn Ops + 'a>) {
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
fn do_flat_command<'a>(str:&str, ops:&mut Box<dyn Ops + 'a>) {
    ops.do_command(str);
}

/// Mapping {
///    "command": String("start\nset verbose 1\n"),
///    "result": String("Change verbose level from \\d{1,} to 1")
/// },
fn do_pmd_command<'a>(map:&Mapping, ops:&mut Box<dyn Ops + 'a>) {
    do_flat_command(&map_str2val(map, "command"), ops);
    if let Some(val) = map.get("result") {
        do_result(&val, ops)
    }
}

fn do_phase_result<'a>(result:&Mapping, ops_db:&mut OpsDb<'a>) {
    for (key, val) in result {
        let tag = key.as_str().unwrap();
        if let Some(ops) = ops_db.get_mut(tag) { do_result(val, ops) }
    }
}

fn do_phase_commands<'a>(phase:&Mapping, ops_db:&mut OpsDb<'a>) {
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
                log::info!(target: "PHASE", "\'{}\' cycle {i} of {repeat}", map_str2val(phase, "name"));
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

    tags.iter().for_each(|t| { commands.insert(&t.app, String::new()); });
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
        println!("# {}> {}", tag, cmdline);
        println!("{cmds}");
    });
}