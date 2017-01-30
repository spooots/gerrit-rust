
//! Implements the gerrit structure

use changes;
use error::GGRResult;
use error::GGRError;
use git2::Repository;
use git2::BranchType;
use std::io::{self, Write};
use std::process::Command;
use url;

/// Interface for Gerrit access.
pub trait GerritAccess {
    /// Returns the (path, query) information
    fn build_url(&self) -> (String, String);
}


/// `Gerrit` structure for management of several gerrit endpoints
pub struct Gerrit {
    url: url::Url,
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
            url: url::Url::parse(&url.into()).unwrap(),
        }
    }

    /// Convenient function to query changes from gerrit server
    ///
    /// `querylist` is used as filter for the call to gerrit. `additional_infos` gives some more
    /// information of one Change entity.
    pub fn changes(&mut self, querylist: Option<Vec<String>>, additional_infos: Option<Vec<String>>)
        -> GGRResult<changes::ChangeInfos>
    {
        let mut changes = changes::Changes::new(&self.url);

        changes.querylist = querylist;
        changes.labellist = additional_infos;

        changes.query_changes()
    }

    /// Convenient function to checkout a topic
    pub fn checkout_topic(&mut self, branchname: &str) -> GGRResult<()> {
            if let Ok(main_repo) = Repository::open(".") {
                let mut out_ok: Vec<String> = Vec::new();
                let mut out_ko: Vec<String> = Vec::new();

                println!("try checkout on main repo ... ");
                match checkout_repo(&main_repo, branchname) {
                    Ok(_) => {
                        let mut base = String::new();

                        base.push_str("OK\ngit submodule update ...");
                        let output_submodule_update = Command::new("git")
                            .arg("submodule")
                            .arg("update")
                            .arg("--recursive")
                            .arg("--init")
                            .output()?;

                        if output_submodule_update.stdout.is_empty() {
                            base.push_str(&format!("  submodule update stdout:\n{}", String::from_utf8_lossy(&output_submodule_update.stdout)));
                        }
                        if output_submodule_update.stderr.is_empty() {
                            base.push_str(&format!("  submodule update stderr:\n{}", String::from_utf8_lossy(&output_submodule_update.stderr)));
                        }
                        out_ok.push(base);
                    },
                    Err(m) => out_ko.push(format!("{} -> {}", main_repo.path().to_str().unwrap(), m.to_string().trim())),
                }

                println!();

                if let Ok(smodules) = main_repo.submodules() {
                    print!("try checkout on submodules ");
                    for smodule in smodules {
                        if let Ok(sub_repo) = smodule.open() {
                            match checkout_repo(&sub_repo, branchname) {
                                Ok(_) => {
                                    print!("+");
                                    out_ok.push(format!("{:?}", smodule.name().unwrap_or("unknown repository")))
                                },
                                Err(m) => {
                                    print!("-");
                                    out_ko.push(format!("{:?} -> {}", smodule.name().unwrap_or("unknown repository"), m.to_string().trim()))
                                },
                            };
                            let _ = io::stdout().flush();
                        }
                    }
                    println!("\n");

                    if !out_ko.is_empty() {
                        println!("Not possible to checkout '{}' on this repositories:", branchname);
                        for entry in out_ko {
                            println!("* {}", entry);
                        }
                    }

                    if !out_ok.is_empty() {
                        println!("\nSuccessfull checkout of '{}' on this repositories:", branchname);
                        for entry in out_ok {
                            println!("* {}", entry);
                        }
                    } else {
                        println!("No checkout happened");
                    }
                }
            }
        Ok(())
    }

    /// Conviention function to fetch topic `topicname` to branch `local_branch_name`.
    ///
    /// If branch exists and `force` is true, the branch is moving to new position.
    pub fn fetch_topic(&mut self, topicname: &str, local_branch_name: &str, force: bool, tracking_branch_name: Option<&str>) -> GGRResult<()> {
        let ofields: Vec<String> = vec!("CURRENT_REVISION".into(), "CURRENT_COMMIT".into());

        let changeinfos = try!(self.changes(Some(vec![format!("topic:{} status:open", topicname)]), Some(ofields)));
        let project_tip = changeinfos.project_tip().unwrap();

        // try to fetch topic for main_repo and all submodules
        'next_ptip: for (p_name, p_tip) in project_tip {
            print!("fetch {} for {} ... ", p_name, p_tip);
            // check for root repository
            if let Ok(main_repo) = Repository::open(".") {
                // check changes on root repository
                match fetch_from_repo(&main_repo, &changeinfos, force, local_branch_name, &p_name, &p_tip, tracking_branch_name) {
                    Ok((true,m)) => {
                        println!("{}", m);
                        continue;
                    },
                    Ok((false, m)) => {
                        println!("KO\n  Error: {}", m.trim());
                    },
                    Err(r) => {
                        // hide all other errors
                        let r = r.to_string();
                        if !r.is_empty() {
                            println!("KO\nError: {}", r.to_string().trim());
                        }
                    }
                };

                // check for submodules
                if let Ok(smodules) = main_repo.submodules() {
                    for smodule in smodules {
                        if let Ok(sub_repo) = smodule.open() {
                            match fetch_from_repo(&sub_repo, &changeinfos, force, local_branch_name, &p_name, &p_tip, tracking_branch_name) {
                                Ok((true, m)) => {
                                    println!("{}", m);
                                    continue 'next_ptip;
                                },
                                Ok((false, m)) => {
                                    println!("KO\n  Error: {}", m.trim());
                                    continue;
                                },
                                Err(r) => {
                                    let r = r.to_string();
                                    if !r.is_empty() {
                                                println!("KO\nError: {}", r.to_string().trim());
                                    }
                                }
                            }
                        } else {
                            println!("{} not opened", smodule.name().unwrap());
                        }
                    }
                }
            }
        }

        Ok(())
    }

}

/// convenient function to checkout a `branch` on a `repo`. If `print_status` is true, messages are
/// printed
fn checkout_repo(repo: &Repository, branchname: &str) -> GGRResult<()> {
    if repo.is_bare() {
        return Err(GGRError::General("repository needs to be a workdir and not bare".into()));
    }

    let output_checkout = try!(Command::new("git")
        .current_dir(repo.workdir().unwrap())
        .arg("checkout")
        .arg(branchname)
        .output());

    if output_checkout.status.success() {
        return Ok(());
    }

    Err(GGRError::General(String::from_utf8_lossy(&output_checkout.stderr).into()))
}

/// convenient function to pull a `p_tip` from a `repo`, if `basename(repo.url)` same as `p_name`
/// is.
///
/// returns `true` if something is pulled, and `false` if no pull was executed. The String object
/// is a status message.
fn fetch_from_repo(repo: &Repository, ci: &changes::ChangeInfos, force: bool, local_branch_name: &str, p_name: &str, p_tip: &str, tracking_branch_name: Option<&str>) -> GGRResult<(bool, String)> {
    if repo.is_bare() {
        return Err(GGRError::General(format!("repository path '{:?}' is bare, we need a workdir", repo.path())));
    }

    for remote_name in repo.remotes().unwrap().iter() {
        if let Ok(remote) = repo.find_remote(remote_name.unwrap()) {
            let url = remote.url().unwrap().to_owned();
            let check_project_names = vec!(
                p_name.into(),
                format!("{}.git", p_name)
            );
            if check_project_names.contains(&String::from(url_to_projectname(&url).unwrap())) {
                if let Ok(entity) = ci.entity_from_commit(p_tip) {
                    if let Some(ref cur_rev) = entity.current_revision {
                        if let Some(ref revisions) = entity.revisions {
                            let reference = &revisions[cur_rev].fetch["http"].reference;
                            let force_string = if force {"+"} else { "" };
                            let refspec = format!("{}{}:{}", force_string, reference, local_branch_name);

                            if !force  && repo.find_branch(local_branch_name, BranchType::Local).is_ok() {
                                // Branch exists, but no force
                                return Ok((false, String::from("Branch exists and no force")));
                            }

                            let mut output_fetch = try!(Command::new("git")
                                .current_dir(repo.path())
                                .arg("fetch")
                                .arg(remote.name().unwrap())
                                .arg(refspec)
                                .output());

                            if output_fetch.status.success() {
                                if let Some(tracking_branch) = tracking_branch_name {
                                    let mut output_tracking = try!(Command::new("git")
                                         .current_dir(repo.path())
                                         .arg("branch")
                                         .arg("--set-upstream-to")
                                         .arg(tracking_branch)
                                         .arg(local_branch_name)
                                         .output());
                                    if !output_tracking.stdout.is_empty() {
                                        output_fetch.stdout.append(&mut String::from("\n* ").into_bytes());
                                        output_fetch.stdout.append(&mut output_tracking.stdout);
                                    }
                                    if !output_tracking.stderr.is_empty() {
                                        output_fetch.stdout.append(&mut String::from("\n* ").into_bytes());
                                        output_fetch.stdout.append(&mut output_tracking.stderr);
                                    }
                                }

                                return Ok((true, try!(String::from_utf8(output_fetch.stdout))));
                            }

                            return Ok((false, try!(String::from_utf8(output_fetch.stderr))));
                        } else {
                            return Err(GGRError::General("no revisions".into()));
                        }
                    } else {
                        return Err(GGRError::General("No cur_rev".into()));
                    }
                } else {
                    return Err(GGRError::General("no entity from commit".into()));
                }
            }
        }
    }

    Err(GGRError::General("".into()))
}

/// returns basename of a project from a url (eg.: https://localhost/test -> test)
fn url_to_projectname(url: &str) -> Option<&str> {
    if let Some(last_slash_at) = url.rfind('/') {
        let (_, remote_project_name) = url.split_at(last_slash_at+1);
        return Some(remote_project_name);
    }
    None
}

#[test]
fn test_url_to_projectname() {
    assert_eq!(url_to_projectname("http://das/haus/vom/nikolause"), Some("nikolause"));
    assert_eq!(url_to_projectname("http://."), Some("."));
    assert_eq!(url_to_projectname("nikolause"), None);
    assert_eq!(url_to_projectname("n/i/k/o/lause"), Some("lause"));
    assert_eq!(url_to_projectname(""), None);
}

