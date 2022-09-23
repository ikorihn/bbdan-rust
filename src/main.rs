use ansi_term::Colour;
use chrono::{DateTime, Local};
use clap::{ArgEnum, Parser, Subcommand};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{default, fs, process};

#[derive(Parser, Debug)]
#[clap(name = "bbrs", version, about, long_about = None, arg_required_else_help = true)]
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

    /// Repo slug
    #[clap(short, long)]
    slug: String,

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

/// サブコマンドの定義
#[derive(Debug, Subcommand)]
enum Commands {
    List,
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
    let slug: String = args.slug;
    let output: Output = args.output;

    let bitbucket = Bitbucket {
        username,
        password,
        workspace,
        slug,
    };

    match args.command {
        Commands::List => {
            list(bitbucket).await;
        }
    }
}

// Bitbucket APIを実行する

const BASE_URL: &str = "https://api.bitbucket.org/2.0";

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

struct Permission {
    object_type: ObjectType,
    alias: String,
    id: String,
    permission: PermissionType,
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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

async fn list(bitbucket: Bitbucket) -> Result<Vec<Permission>, Box<dyn std::error::Error>> {
    let mut permissions: Vec<Permission> = Vec::new();

    let url = format!(
        r#"{}/repositories/{}/{}/permissions-config/groups"#,
        BASE_URL, bitbucket.workspace, bitbucket.slug,
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .basic_auth(&bitbucket.username, Some(&bitbucket.password))
        .send()
        .await?;

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

    let url_users = format!(
        r#"{}/repositories/{}/{}/permissions-config/users"#,
        BASE_URL, bitbucket.workspace, bitbucket.slug,
    );
    let resp_users = client
        .get(url_users)
        .basic_auth(bitbucket.username, Some(bitbucket.password))
        .send()
        .await?;

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

    for p in &permissions {
        println!(
            "{:?}, {:?}, {:?}, {:?}",
            p.object_type, p.id, p.alias, p.permission,
        );
    }

    Ok(permissions)
}
