
//! configuration related

use clap::{self, SubCommand, App};
use gerritlib::error::GGRError;
use gerritlib::error::GGRResult;
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use toml_config;

pub fn menu<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("config")
    .about("config management for ggr")
    .subcommand(SubCommand::with_name("list")
                .help("List all config options")
    )
}

/// manage subfunction of `config` command
///
/// Currently implemented sub commands:
///
/// * list
pub fn manage(x: &clap::ArgMatches) -> GGRResult<()> {
    match x.subcommand() {
        ("list", Some(y)) => { list(y) },
        _ => {
            println!("{}", x.usage());
            Ok(())
        },
    }
}

fn list(_: &clap::ArgMatches) -> GGRResult<()> {
    let cf = try!(ConfigFile::discover(".", ".ggr.conf"));
    let c = Config::from_configfile(cf);

    println!("{}", c);

    Ok(())
}

/// Holds configuration for gerrit
#[derive(RustcDecodable, RustcEncodable)]
pub struct Config {
    /// scheme like http, https etc
    scheme: String,
    /// base of url. Eg. localhost
    base: String,
    /// port of the http endpoint (defaults to 80)
    port: u16,
    /// a root url path
    appendix: String,
    /// username to login
    username: String,
    /// password for login
    password: String,
    /// claims the repository as the topmost repository
    root: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            scheme: "".to_owned(),
            base: "".to_owned(),
            port: 80,
            appendix: "".to_owned(),
            username: "".to_owned(),
            password: "".to_owned(),
            root: true,
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(writeln!(f, "* url ......... : {sche}://{base}:{port}/{appe}",
               sche = self.scheme.clone(),
               base = self.base.clone(),
               port = self.port.clone(),
               appe = self.appendix.clone(),
        ));
        try!(writeln!(f, "  user/pass ... : {user} / \"{pass}\"",
               user = self.username.clone(),
               pass = self.password.clone(),
        ));

        write!(f, "  root ........ : {root}", root = self.root.clone())
    }
}

impl Config {
    /// Creates new Config from ConfigFile
    pub fn from_configfile(cf: ConfigFile) -> Config {
        toml_config::ConfigFactory::load(cf.file.path().as_path())
    }

    /// Config is only functional if `base` and `scheme` are set. This function returns `false` if
    /// one of them is not set.
    pub fn is_valid(&self) -> bool {
        if self.scheme.is_empty() || self.base.is_empty() {
            return false;
        }

        true
    }

    pub fn get_username(&self) -> &str {
        &self.username
    }

    pub fn get_password(&self) -> &str {
        &self.password
    }

    pub fn get_base_url(&self) -> String {
        let appe = if self.appendix == "" {
            "".to_string()
        } else {
            format!("{}/", self.appendix)
        };
        format!("{scheme}://{base}:{port}/{appe}",
                scheme = self.scheme,
                base = self.base,
                port = self.port,
                appe = appe
        )
    }
}

/// Represents a config file filesystem object
pub struct ConfigFile {
    file: fs::DirEntry,
}

impl fmt::Display for ConfigFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.file.file_name())
    }
}

impl ConfigFile {
    /// returns configfile
    fn new_with_path(p: fs::DirEntry) -> ConfigFile {
        ConfigFile {
            file: p,
        }
    }

    /// search current work directory and all parent folder for a config file.
    ///
    /// folder: path where to search. Use `.` for current directory.
    /// name: name of config file
    pub fn discover(folder: &str, name: &str) -> GGRResult<ConfigFile> {
        let cwd = try!(env::current_dir());
        let mut folder = folder;

        if folder.eq(".") {
            folder = match cwd.to_str().ok_or_else(|| GGRError::General("something is wrong with current directory".to_string())) {
                Ok(x) => x,
                Err(x) => return Err(x),
            }
        }

        let mut path = Path::new(folder.into());
        let pathtemp = path;


        loop {
            /* check folder for `name` */
            for entry in try!(fs::read_dir(path)) {
                let entry = try!(entry);
                if entry.file_name() == *name {
                    return Ok(ConfigFile::new_with_path(entry));
                }
            }

            /* not found? ... Go up to parent and check again */
            match path.parent() {
                Some(x) => {
                    path = x;
                },
                None => {
                    break;
                }
            }
        }

        Err(GGRError::StdIo(io::Error::new( io::ErrorKind::NotFound,
                            format!("conf file {} in {:?} and all parent directories not found", name, pathtemp)
                            )
            )
        )
    }

    /* TODO: write convience function discover_root */
}

