
//! Implements the gerrit structure

use call::Call;
use entities::*;
use entities;
use error::GGRError;
use error::GGRResult;
use rustc_serialize;
use std::error::Error;


/// `Gerrit` structure for management of several gerrit endpoints
pub struct Gerrit {
    call: Call,
}

impl Gerrit {
    /// Creates a new `Gerrit` object
    ///
    /// The url points to the http endpoint of an gerrit server like
    /// `http://localhost:8080/gerrit`. All other function append to this url there endpoint pathes
    /// and query parameters.
    pub fn new<S>(url: S) -> Gerrit
    where S: Into<String> {
        Gerrit {
            call: Call::new(url.into()),
        }
    }

    /// pull changes from gerrit server
    ///
    /// `querylist` and `additional_info` are used as filter in the call to gerrit.
    pub fn changes(&mut self, querylist: Option<&Vec<String>>, additional_infos: Option<&[&str]>, username: &str, password: &str)
        -> GGRResult<entities::ChangeInfos>
    {
        let mut querystring = "pp=0&q=".to_string();
        match querylist {
            None => { /* nothing to do, we call without filter */ },
            Some(x) => {
                let urlfragment = Changes::build_url(&x);
                querystring = format!("{}{}", querystring, urlfragment);
            },
        };

        if !username.is_empty() && !password.is_empty() {
            self.call.set_credentials(username, password);
        }

        if let Ok(cr) = self.call.get("/changes/".into(), querystring) {
            let body = match cr.body {
                Some(x) => x,
                None => {
                    /* no body content */
                    return Ok(entities::ChangeInfos::new());
                }
            };
            let data2 = body.iter().fold(String::from(""), |news, el| format!("{}{}", news, el));

            let data4: Vec<ChangeInfo> = match rustc_serialize::json::decode(&data2) {
                Ok(d) => {
                    d
                },
                Err(err) => {
                    return Err(GGRError::General(format!("{}: {}", err.description(), data2)));
                },
            };

            let data5 = match  rustc_serialize::json::Json::from_str(&data2) {
                Ok(d) => d,
                Err(e) => {
                    println!("error: {}",e);
                    return Err(GGRError::from(e));
                }
            };

            let changeinfos = entities::ChangeInfos::new_with_data(data4, Some(data5));

            return Ok(changeinfos);
        } else {
            println!("call problem");
        }
        Ok(entities::ChangeInfos::new())
    }
}

// helper structures
struct Changes;
impl Changes {
    pub fn build_url(querylist: &Vec<String>) -> String {
        let mut out = String::new();
        for el in querylist.iter() {
            out.push_str(el);
            out.push_str("+");
        }
        if let Some(x) = out.chars().last() {
            if x == '+' {
                out = out.trim_right_matches(x).to_string();
            }
        };

        out
    }
}

#[test]
fn test_changes_build_url() {
    assert_eq!(Changes::build_url(&vec!()), "");
    assert_eq!(Changes::build_url(&vec!("a:1", "b:2")), "a:1+b:2");
    assert_eq!(Changes::build_url(&vec!("a:1")), "a:1");
}
