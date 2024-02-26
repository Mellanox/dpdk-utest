use clap::{Arg, ArgAction, ArgMatches, Command};

pub struct CmdLine {
    pub commands: String,
    pub hosts:String,
    pub mtdev:String,
    pub verbose:bool,
    pub show:bool,
    pub reuse_conifiguration:bool,
}

impl Default for CmdLine {
    fn default() -> Self {
        Self {
            commands: Default::default(),
            hosts: Default::default(),
            mtdev: Default::default(),
            verbose: false,
            show: false,
            reuse_conifiguration: false,
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
            .arg(Arg::new("verbose")
                .long("verbose")
                .short('v')
                .required(false)
                .action(ArgAction::SetTrue)
                .help("verbose execution"))
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
            ;
        let params = cmd.get_matches();

        CmdLine {
            commands: CmdLine::fetch_param_value(&params, "commands"),
            hosts: CmdLine::fetch_param_value(&params, "hosts"),
            mtdev: if params.contains_id("mtdev") {
                CmdLine::fetch_param_value(&params, "mtdev")
            } else {
                String::new()
            },
            verbose: *params.get_one::<bool>("verbose").unwrap(),
            show: *params.get_one::<bool>("show").unwrap(),
            reuse_conifiguration: *params.get_one::<bool>("fast").unwrap(),
        }
    }
}
