use ansi_term::Colour;
use chrono::{DateTime, Local};
use clap::{ArgEnum, Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use reqwest::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{default, fs, process};

#[derive(Parser, Debug)]
#[clap(name = "bbdan", version, about, long_about = None, arg_required_else_help = true)]
struct Args {
    /// Username
    #[clap(short, long, value_name = "USERNAME")]
    username: String,

    /// App password
    #[clap(short, long, value_name = "APP PASSWORD")]
    password: String,

    /// Workspace
    #[clap(short, long, value_name = "WORKSPACE")]
    workspace: String,

    /// Output type
    #[clap(
        short,
        long,
        arg_enum,
        value_name = "OUTPUT TYPE",
        default_value = "text"
    )]
    output: Output,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, ArgEnum, Copy)]
enum Output {
    Csv,
    Json,
    Text,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List permission of repo
    List { repo: String },
    /// Copy permission setting from src_repo to dest_repo
    Copy { src_repo: String, dest_repo: String },
    /// Remove permission
    Remove { repo: String },
}

struct OutputMessage {
    datetime: DateTime<Local>,
    url: String,
    status_code: StatusCode,
    elapsed: Duration,
}

impl OutputMessage {
    fn new(
        datetime: DateTime<Local>,
        url: String,
        status_code: StatusCode,
        elapsed: Duration,
    ) -> Self {
        Self {
            datetime,
            url,
            status_code,
            elapsed,
        }
    }

    fn to_formatted(&self, output: Output) -> String {
        let dt = self.datetime.format("%Y-%m-%d %H:%M:%S").to_string();
        let url = self.url.as_str().to_string();
        let st = self.status_code.to_string();
        let response_time = format!(
            "{}.{:03}",
            self.elapsed.as_secs(),
            self.elapsed.subsec_nanos() / 1_000_000
        );

        match output {
            Output::Csv => {
                format!(r#""{}","{}","{}","{}""#, dt, url, st, response_time)
            }
            Output::Json => {
                format!(
                    r#"{{"datetime": "{}","url: "{}","statusCode": "{}","responseTime": "{}"}}"#,
                    dt, url, st, response_time
                )
            }
            Output::Text => {
                format!("{} {} {} {}", dt, url, st, response_time)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let username: String = args.username;
    let password: String = args.password;
    let workspace: String = args.workspace;
    let output: Output = args.output;

    match args.command {
        Commands::List { repo } => {
            let bitbucket = Bitbucket {
                username: username.to_string(),
                password: password.to_string(),
                workspace: workspace.to_string(),
                slug: repo.to_string(),
            };

            let result = list(bitbucket).await;
            println!("Repository: {}", repo.to_string());
            for p in &result.ok().unwrap() {
                println!(
                    "{:?}, {:?}, {:?}, {:?}",
                    p.object_type, p.id, p.alias, p.permission,
                );
            }
        }
        Commands::Copy {
            src_repo,
            dest_repo,
        } => {
            let src = Bitbucket {
                username: username.to_string(),
                password: password.to_string(),
                workspace: workspace.to_string(),
                slug: src_repo,
            };
            let dest = Bitbucket {
                username: username.to_string(),
                password: password.to_string(),
                workspace: workspace.to_string(),
                slug: dest_repo,
            };
            let result = copy(src, dest).await;
            result.ok();
        }
        Commands::Remove { repo } => {
            let bitbucket = Bitbucket {
                username: username.to_string(),
                password: password.to_string(),
                workspace: workspace.to_string(),
                slug: repo.to_string(),
            };

            let result = remove(bitbucket).await;
            result.ok();
        }
    }
}

// Bitbucket APIを実行する

const BASE_URL: &str = "https://api.bitbucket.org/2.0";

#[derive(Debug, Clone)]
struct Bitbucket {
    username: String,
    password: String,
    workspace: String,
    slug: String,
}

// #[derive(Debug, Serialize, Deserialize)]
// struct GroupPermissions {
//     values: Vec<GroupPermission>
// }
// #[derive(Debug, Serialize, Deserialize)]
// struct GroupPermission {
//     permission: String,
//
// }

#[derive(Debug, Clone)]
struct Permission {
    object_type: ObjectType,
    alias: String,
    id: String,
    permission: PermissionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectType {
    User,
    Group,
}

fn object_type_from_str(s: &str) -> ObjectType {
    match s {
        "user" => return ObjectType::User,
        "group" => return ObjectType::Group,
        _ => return ObjectType::User,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionType {
    Read,
    Write,
    Admin,
}
fn permission_type_from_str(s: &str) -> PermissionType {
    match s {
        "read" => return PermissionType::Read,
        "write" => return PermissionType::Write,
        "admin" => return PermissionType::Admin,
        _ => return PermissionType::Read,
    }
}
fn permission_type_to_str(p: PermissionType) -> String {
    match p {
        PermissionType::Read => return String::from("read"),
        PermissionType::Write => return String::from("write"),
        PermissionType::Admin => return String::from("admin"),
        _ => return String::from("read"),
    }
}

struct BitbucketClient {
    http_client: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}
impl BitbucketClient {
    fn new(
        http_client: reqwest::Client,
        base_url: String,
        username: String,
        password: String,
    ) -> Self {
        Self {
            http_client,
            base_url,
            username,
            password,
        }
    }

    async fn http_get(&self, url: String) -> Result<Response, reqwest::Error> {
        let full_url = format!(r#"{}/{}"#, self.base_url, url);
        let resp = self
            .http_client
            .get(full_url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await;
        return resp;
    }
}

async fn list(bitbucket: Bitbucket) -> Result<Vec<Permission>, Box<dyn std::error::Error>> {
    let mut permissions: Vec<Permission> = Vec::new();

    let client = BitbucketClient::new(
        reqwest::Client::new(),
        BASE_URL.to_string(),
        bitbucket.username,
        bitbucket.password,
    );

    let resp = client
        .http_get(format!(
            r#"repositories/{}/{}/permissions-config/groups"#,
            bitbucket.workspace, bitbucket.slug,
        ))
        .await
        .unwrap();

    if !resp.status().is_success() {
        println!("failed to get permission");
        return Ok(permissions);
    }

    let permission_groups: Value = resp.json().await?;

    for v in permission_groups["values"].as_array().unwrap() {
        let p = Permission {
            permission: permission_type_from_str(v["permission"].as_str().unwrap()),
            object_type: object_type_from_str(v["group"]["type"].as_str().unwrap()),
            alias: String::from(v["group"]["name"].as_str().unwrap()),
            id: String::from(v["group"]["slug"].as_str().unwrap()),
        };
        permissions.push(p);
    }

    let resp_users = client
        .http_get(format!(
            r#"repositories/{}/{}/permissions-config/users"#,
            bitbucket.workspace, bitbucket.slug,
        ))
        .await
        .unwrap();

    if !resp_users.status().is_success() {
        println!("failed to get permission");
        return Ok(vec![]);
    }

    let permission_users: Value = resp_users.json().await?;

    for v in permission_users["values"].as_array().unwrap() {
        let p = Permission {
            permission: permission_type_from_str(v["permission"].as_str().unwrap()),
            object_type: object_type_from_str(v["user"]["type"].as_str().unwrap()),
            alias: String::from(v["user"]["nickname"].as_str().unwrap()),
            id: String::from(v["user"]["uuid"].as_str().unwrap()),
        };
        permissions.push(p);
    }

    Ok(permissions)
}

async fn copy(
    src: Bitbucket,
    dest: Bitbucket,
) -> Result<Vec<Permission>, Box<dyn std::error::Error>> {
    let permissions_src = list(src).await.ok().unwrap();
    let permissions_before = list(dest.clone()).await.ok().unwrap();

    let mut dest_ids: HashMap<String, &Permission> = HashMap::new();
    for p in &permissions_before {
        dest_ids.insert(p.id.to_string(), &p);
    }

    let mut src_ids: HashSet<String> = HashSet::new();
    let client = reqwest::Client::new();
    for p in permissions_src {
        src_ids.insert(p.id.to_string());

        if dest_ids.contains_key(&p.id) {
            let dests = dest_ids.get(&p.id).unwrap();
            if p.permission == dests.permission {
                println!("Not change: id={}, name={}", p.id, p.alias);
                continue;
            } else {
                let message = format!(
                    "Permission update: id={}, name={}, before={}, after={}. Continue?",
                    p.id,
                    p.alias,
                    permission_type_to_str(p.permission),
                    permission_type_to_str(dests.permission),
                );
                if Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(message)
                    .default(true)
                    .wait_for_newline(true)
                    .interact()
                    .unwrap()
                {
                    println!("Continue");
                } else {
                    println!("Skip");
                    continue;
                }
            }
        } else {
            let message = format!("Add: id={}, name={}. Continue?", p.id, p.alias);
            if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(message)
                .default(true)
                .wait_for_newline(true)
                .interact()
                .unwrap()
            {
                println!("Continue");
            } else {
                println!("Skip");
                continue;
            }
        }

        let url = if p.object_type == ObjectType::User {
            format!(
                r#"{}/repositories/{}/{}/permissions-config/users/{}"#,
                BASE_URL, dest.workspace, dest.slug, p.id,
            )
        } else {
            format!(
                r#"{}/repositories/{}/{}/permissions-config/groups/{}"#,
                BASE_URL, dest.workspace, dest.slug, p.id,
            )
        };

        let mut map = HashMap::new();
        map.insert("permission", permission_type_to_str(p.permission));

        println!("PUT {}", url);

        let resp = client
            .put(url)
            .basic_auth(&dest.username, Some(&dest.password))
            .json(&map)
            .send()
            .await?;

        if !resp.status().is_success() {
            println!("failed to request");
            return Ok(vec![]);
        }

        let result: Value = resp.json().await?;
        println!("result: {}", result);
    }

    for p in permissions_before {
        if src_ids.contains(&p.id) {
            continue;
        }

        let message = format!("Remove: id={}, name={}. Continue?", p.id, p.alias);
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(true)
            .wait_for_newline(true)
            .interact()
            .unwrap()
        {
            println!("Continue");
        } else {
            println!("Skip");
            continue;
        }
        let url = if p.object_type == ObjectType::User {
            format!(
                r#"{}/repositories/{}/{}/permissions-config/users/{}"#,
                BASE_URL, dest.workspace, dest.slug, p.id,
            )
        } else {
            format!(
                r#"{}/repositories/{}/{}/permissions-config/groups/{}"#,
                BASE_URL, dest.workspace, dest.slug, p.id,
            )
        };

        println!("DELETE {}", url);

        let resp = client
            .delete(url)
            .basic_auth(&dest.username, Some(&dest.password))
            .send()
            .await?;

        if !resp.status().is_success() {
            println!("failed to request");
            return Ok(vec![]);
        }

        let result: Value = resp.json().await?;
        println!("result: {}", result);
    }

    let permissions_after = list(dest).await.ok().unwrap();
    Ok(permissions_after)
}

async fn remove(bitbucket: Bitbucket) -> Result<(), Box<dyn std::error::Error>> {
    let permissions = list(bitbucket.clone()).await.ok().unwrap();

    let multiselected: Vec<String> = permissions
        .iter()
        .map(|x| {
            format!(
                "{:?} - {:?} - {:?} - {:?}",
                x.object_type, x.id, x.alias, x.permission
            )
        })
        .collect();

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Pick permission you want to remove")
        .items(&multiselected[..])
        .interact()
        .unwrap();

    if selections.is_empty() {
        println!("You did not select anything :(");
    } else {
        let client = reqwest::Client::new();

        for selection in selections {
            let p = permissions[selection].clone();

            let url = if p.object_type == ObjectType::User {
                format!(
                    r#"{}/repositories/{}/{}/permissions-config/users/{}"#,
                    BASE_URL, bitbucket.workspace, bitbucket.slug, p.id,
                )
            } else {
                format!(
                    r#"{}/repositories/{}/{}/permissions-config/groups/{}"#,
                    BASE_URL, bitbucket.workspace, bitbucket.slug, p.id,
                )
            };

            let resp = client
                .delete(url)
                .basic_auth(&bitbucket.username, Some(&bitbucket.password))
                .send()
                .await?;

            if !resp.status().is_success() {
                println!("failed to request");
                return Ok(());
            }

            let result: Value = resp.json().await?;
            println!("result: {}", result);
        }
    };

    Ok(())
}
