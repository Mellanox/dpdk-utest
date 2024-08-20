use std::env;
use clap::{Arg, ArgAction, ArgMatches, Command};

pub struct CmdLine {
    pub commands_file:String,
    pub hosts_file:String,
    pub mlx_dev:String,
    pub ssh_key:String,
    pub verbose:bool,
    pub silent:bool,
    pub show_commands:bool,
    pub reuse_conifiguration:bool,
    pub loopback:bool,
}

impl Default for CmdLine {
    fn default() -> Self {
        Self {
            commands_file: Default::default(),
            hosts_file: Default::default(),
            mlx_dev: Default::default(),
            ssh_key: Default::default(),
            verbose: false,
            silent: false,
            show_commands: false,
            reuse_conifiguration: false,
            loopback: false,
        }
    }
}

impl CmdLine {
    fn fetch_param_value(params:&ArgMatches, key:&str) -> String {
        if let Some(val) = params.get_one::<String>(key) { val.to_string() }
        else { String::new() }
    }

}

impl CmdLine {
    pub fn new() -> Self {
        let cmd = Command::new("ursa")
            .arg(Arg::new("commands")
                .long("commands")
                .required(false)
                .action(ArgAction::Set)
                .help("YAML file with test commands"))
            .arg(Arg::new("hosts")
                .long("hosts")
                .required(false)
                .action(ArgAction::Set)
                .help("YAML file with hosts for the test"))
            .arg(Arg::new("mtdev")
                .long("mtdev")
                .required(false)
                .action(ArgAction::Set)
                .help("MT device"))
            .arg(Arg::new("key")
                .long("key")
                .required(false)
                .action(ArgAction::Set)
                .help("SSH key"))
            .arg(Arg::new("verbose")
                .long("verbose")
                .short('v')
                .required(false)
                .action(ArgAction::SetTrue)
                .help("verbose execution"))
            .arg(Arg::new("silent")
                .long("silent")
                .short('s')
                .required(false)
                .action(ArgAction::SetTrue)
                .help("silent execution"))
            .arg(Arg::new("fast")
                .long("fast")
                .required(false)
                .action(ArgAction::SetTrue)
                .help("reuse existing hosts configuration"))
            .arg(Arg::new("show")
                .long("show")
                .required(false)
                .action(ArgAction::SetTrue)
                .help("show test commands and exit"))
            .arg(Arg::new("loopback")
                .long("loopback")
                .required(false)
                .action(ArgAction::SetTrue)
                .help("loopback setup"))
            ;
        let params = cmd.get_matches();

        CmdLine {
            commands_file: CmdLine::fetch_param_value(&params, "commands"),
            hosts_file: CmdLine::fetch_param_value(&params, "hosts"),
            mlx_dev: if params.contains_id("mtdev") {
                CmdLine::fetch_param_value(&params, "mtdev")
            } else {
                String::new()
            },
            ssh_key: if params.contains_id("key") {
                CmdLine::fetch_param_value(&params, "key")
            } else {
                format!("{}/.ssh/id_rsa",env::var("HOME").unwrap())
            },
            verbose: *params.get_one::<bool>("verbose").unwrap(),
            silent: *params.get_one::<bool>("silent").unwrap(),
            show_commands: *params.get_one::<bool>("show").unwrap(),
            reuse_conifiguration: *params.get_one::<bool>("fast").unwrap(),
            loopback: *params.get_one::<bool>("loopback").unwrap(),
        }
    }
}
