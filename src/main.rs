use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::shells::{Bash, Fish, PowerShell, Zsh};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use indicatif::{ProgressBar, ProgressStyle};
use prost::Message;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use filz::behavior::BehaviorProfile;
use filz::cache::SearchCache;
use filz::models::{AppError, AppResult, Item, Registry};
use filz::proto as pb;
use filz::reporting::{render_banner, render_table};

mod config {
    use std::path::PathBuf;

    pub fn workspace_root() -> PathBuf {
        // 1. Environment variable override
        if let Ok(v) = std::env::var("FILE_CLI_ROOT") {
            let p = PathBuf::from(v);
            if p.is_dir() {
                return p;
            }
        }
        // 2. Platform default
        default_workspace()
    }

    fn default_workspace() -> PathBuf {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            // ~/.file-cli as sensible default
            dirs_home().join(".file-cli")
        } else if cfg!(target_os = "windows") {
            dirs_home().join(".file-cli")
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".file-cli")
        }
    }

    fn dirs_home() -> PathBuf {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    pub fn registry_path() -> PathBuf {
        workspace_root().join(".registry.pb")
    }

    pub fn legacy_registry_path() -> PathBuf {
        workspace_root().join(".registry.json")
    }

    pub fn vendor_bin(name: &str) -> Option<std::path::PathBuf> {
        // 1. Check vendor/ relative to cwd
        let vendor = std::env::current_dir()
            .ok()
            .map(|d| d.join("vendor").join(name))
            .filter(|p| p.exists());
        if vendor.is_some() {
            return vendor;
        }
        // 2. Fallback: system PATH
        std::process::Command::new(name)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|_| which(name))
    }

    fn which(name: &str) -> Option<std::path::PathBuf> {
        let cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        std::process::Command::new(cmd)
            .arg(name)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .map(|s| std::path::PathBuf::from(s.trim()))
            })
    }
}

const APP_NAME: &str = "filz";
const APP_VERSION: &str = "2.0.1";

const LOGO: &str = r#"
    ____
   |  _ \ _____   _____ _ __ _ __   ___
   | |_) / _ \ \ /\ / / _ \ '__| '_ \ / _ \
   |  _ <  __/\ V  V /  __/ |  | |_) |  __/
   |_| \_\___| \_/\_/ \___|_|  | .__/ \___|
"#;

fn header(title: &str) -> String {
    let trunc = if title.len() > 30 {
        &title[..30]
    } else {
        title
    };
    let prefix = format!("╔══ {} ", trunc);
    let padding = (48usize).saturating_sub(prefix.len() + 1);
    let rule = "═".repeat(padding);
    format!("{}{}╗", prefix, rule)
}

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb
}

fn with_spinner<T, F: FnOnce() -> AppResult<T>>(msg: &str, work: F) -> AppResult<T> {
    let pb = spinner(msg);
    match work() {
        Ok(v) => {
            pb.finish_and_clear();
            println!("{} {}", "✓".green().bold(), msg);
            Ok(v)
        }
        Err(e) => {
            pb.finish_and_clear();
            println!("{} {}", "✗".red().bold(), msg);
            Err(e)
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = APP_NAME,
    version = APP_VERSION,
    about = "จัดการ workspace — สร้าง ค้นหา จัดเก็บ ทำความสะอาด",
    long_about = "filz คือ CLI ศูนย์กลาง workspace สำหรับจัดการ projects, assets, docs และ archive\n\nคำสั่งหลัก:\n  new  — สร้างรายการใหม่ (wizard แบบ step-by-step)\n  list — แสดงรายการทั้งหมด (กรองตามประเภท)\n  find  — ค้นหารายการใน registry\n  glob  — ค้นหารายการตามชื่อ\n  grep  — ค้นหาข้อความในไฟล์\n  archive — บีบอัดและเก็บเข้าหมวด archive\n  clean — ล้างไฟล์ขยะ (preview ก่อนเสมอ)\n\nคำสั่งขั้นสูง: vendor, benchmark, dedup, system, completion\n\nพิมพ์ 'filz --completion bash' เพื่อตั้ง auto-complete ใน shell",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct Cli {
    #[arg(short = 'h', long = "help", action = ArgAction::Help, help = "แสดง help")]
    help: Option<bool>,

    #[arg(short = 'v', long = "version", action = ArgAction::Version, help = "แสดง version")]
    version: Option<bool>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// สร้างรายการใหม่แบบ wizard (step-by-step)
    /// เลือกประเภทงาน → เลือก template → ดู path ที่จะสร้าง → ยืนยัน
    #[command(alias = "n")]
    New {
        /// ตั้งชื่อรายการ (ข้ามได้ จะถามแบบ interactive)
        #[arg(long)]
        name: Option<String>,
        /// ประเภทงาน: work, doc, asset, archive, tool, personal, client, lab
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,
        /// template ที่ต้องการใช้ (เช่น website, app, blog, note)
        #[arg(long, value_name = "TEMPLATE")]
        template: Option<String>,
        /// ข้าม wizard และสร้างทันที (ใช้เมื่อระบุ --name, --type ครบแล้ว)
        #[arg(long)]
        yes: bool,
    },

    /// เพิ่ม item เข้า registry เพื่อให้ filz เฝ้าดู path หรือ subfolder ต่อไป
    Add {
        /// ชื่อรายการใน registry
        #[arg(long)]
        name: String,
        /// path ใหม่สำหรับ item ใด ๆ ที่ต้องการเฝ้าดู
        #[arg(long, value_name = "PATH")]
        path: Option<PathBuf>,
        /// เพิ่ม subfolder ใน item ที่อยู่ใน registry (เช่น --part src)
        #[arg(long, value_name = "FOLDER")]
        part: Option<String>,
    },

    /// ตั้งขอบเขตของ item ที่ filz จะจัดการ/เฝ้าดู
    /// ใช้เพื่อปรับ scope ของ item ไม่ใช่แค่ set ค่าทั่วไป
    #[command(alias = "scope")]
    Set {
        /// ชื่อ item
        #[arg(long)]
        name: String,
        /// ปรับ type ของ item ที่กำลังเฝ้าดู
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,
        /// ปรับ path ของ item ที่กำลังเฝ้าดู
        #[arg(long, value_name = "PATH")]
        path: Option<PathBuf>,
    },

    /// แสดงรายการ (browse-first)
    /// แสดงเฉพาะรายการที่ยัง active ซ่อน archive โดยอัตโนมัติ
    #[command(alias = "ls")]
    List {
        /// แสดงทั้งหมดรวม archive
        #[arg(long, action = ArgAction::SetTrue)]
        all: bool,
        /// กรองตามประเภท: work, doc, asset, archive, tool
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,
    },

    /// ค้นหารายการ (search-first)
    /// ค้นหาจากชื่อ, ประเภท, path หรือ template
    #[command(alias = "f")]
    Find {
        /// คำค้นหา
        query: String,
        /// แสดงผลละเอียด (แสดง stack และ path เต็ม)
        #[arg(long)]
        verbose: bool,
    },

    /// ค้นหาไฟล์ตามชื่อ (find files)
    /// ค้นหาไฟล์ใน workspace ตามรูปแบบชื่อ เช่น "*.ts", "main*"
    #[command(alias = "g")]
    Glob {
        /// รูปแบบชื่อไฟล์ (ใช้ * เป็นตัวแทนอักขระใดๆ)
        pattern: String,
        /// ค้นหาในไดเรกทอรีนี้ (default: ไดเรกทอรีปัจจุบัน)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// ค้นหาข้อความในไฟล์ (search text)
    /// ค้นหาคำหรือประโยคในเนื้อหาไฟล์ (ใช้ rg ถ้ามี, ไม่งั้นใช้ built-in)
    #[command(alias = "gr")]
    Grep {
        /// คำหรือรูปแบบที่ต้องการค้นหา
        pattern: String,
        /// ค้นหาในไดเรกทอรีนี้ (default: ไดเรกทอรีปัจจุบัน)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// บีบอัดและเก็บเข้าหมวด archive (preview ก่อนเสมอ)
    Archive {
        /// ชื่อรายการใน registry ที่ต้องการ archive
        #[arg(long)]
        name: Option<String>,
        /// บีบอัดไดเรกทอรีนี้โดยไม่ต้องมีใน registry
        #[arg(long)]
        path: Option<PathBuf>,
        /// รูปแบบไฟล์: zip, tar, targz
        #[arg(long, value_enum, default_value = "zip")]
        format: ArchiveFormat,
        /// บันทึกไปยัง path นี้ (default: 04-archive/)
        #[arg(long)]
        output: Option<PathBuf>,
        /// สร้าง archive ทันทีโดยไม่ถามยืนยัน
        #[arg(long, short)]
        yes: bool,
    },

    /// ล้างไฟล์ขยะ (preview ก่อนเสมอ)
    Clean {
        /// ทำความสะอาดทันทีโดยไม่ถามยืนยัน
        #[arg(short, long)]
        yes: bool,
    },

    /// ตรวจสุขภาพระบบ
    Doctor,

    /// จัดการระบบ (ขั้นสูง)
    System {
        #[command(subcommand)]
        action: SystemCmd,
    },

    /// หาเครื่องมือ (find tools)
    /// ค้นหาเครื่องมือใน vendor/, PATH, apt หรือ docs
    Vendor {
        #[command(subcommand)]
        action: VendorCmd,
    },

    /// จัดการ git
    Git {
        #[arg(long)]
        status: bool,
        #[arg(long)]
        add: bool,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long)]
        push: bool,
        #[arg(long)]
        pull: bool,
        #[arg(long)]
        log: bool,
    },

    /// ตรวจ toolchain ที่ติดตั้งอยู่
    Tools,

    /// ตรวจความถูกต้องของ registry (path มีอยู่จริง, ไม่ซ้ำ)
    Check,

    /// แสดงไฟล์ขนาดใหญ่ top N
    Size {
        /// จำนวนรายการที่แสดง (default: 20)
        #[arg(long, default_value = "20")]
        top: usize,
        /// ค้นหาในไดเรกทอรีนี้ (default: ไดเรกทอรีปัจจุบัน)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// สแกนและจัดการไฟล์ซ้ำ (preview ก่อนเสมอ)
    Duplicate {
        /// สแกนในไดเรกทอรีนี้ (default: ไดเรกทอรีปัจจุบัน)
        #[arg(long)]
        path: Option<PathBuf>,
        /// ทำการทันที: delete, move, copy (default: แสดงเฉพาะ preview)
        #[arg(long, value_name = "ACTION")]
        action: Option<String>,
    },

    /// สแกนและลบไฟล์ซ้ำ (preview ก่อนเสมอ)
    Dedup {
        /// สแกนในไดเรกทอรีนี้ (default: ไดเรกทอรีปัจจุบัน)
        #[arg(long)]
        path: Option<PathBuf>,
        /// ลบไฟล์ซ้ำทันที (ไม่ถามยืนยัน)
        #[arg(long, short)]
        yes: bool,
    },

    /// แสดง template ที่มี (ใช้กับ `filz new`)
    Templates,

    /// จัดการ ignore patterns สำหรับการค้นหา
    Ignore {
        #[arg(long)]
        list: bool,
        #[arg(long)]
        add: Option<String>,
        #[arg(long)]
        remove: Option<String>,
    },

    /// สร้าง shell completion (advertised: ลดการจำ syntax)
    /// ติดตั้ง auto-complete สำหรับ Bash, Fish, Zsh, PowerShell
    Completion {
        /// Shell ที่ต้องการ: bash, fish, zsh, powershell
        #[arg(value_enum)]
        shell: Shell,
        /// บันทึก script ที่สร้างลงไฟล์ แทนแสดงใน stdout
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
    },

    /// ทดสอบประสิทธิภาพระบบ (benchmark)
    Benchmark,
}

#[derive(Subcommand, Debug)]
enum VendorCmd {
    /// แสดง vendor tools ที่มี
    List,
    /// ค้นหา tool จาก URL
    Search {
        #[arg(long)]
        name: String,
    },
    /// บันทึก tool info จาก URL เป็น markdown
    Store {
        #[arg(long)]
        name: String,
        #[arg(long)]
        url: String,
    },
    /// ลบ vendor tool ที่ซ้ำ
    Dedup,
}

#[derive(Subcommand, Debug)]
enum SystemCmd {
    /// ดูสถานะระบบ
    Status,
    /// ล้าง cache แบบ dry-run หรือจริงถ้า --yes
    Clean {
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum Shell {
    Bash,
    Fish,
    Zsh,
    PowerShell,
}

#[derive(ValueEnum, Clone, Debug)]
enum ArchiveFormat {
    Zip,
    Tar,
    Targz,
    Sevenz,
}

impl std::fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ArchiveFormat::Zip => write!(f, "zip"),
            ArchiveFormat::Tar => write!(f, "tar"),
            ArchiveFormat::Targz => write!(f, "tar.gz"),
            ArchiveFormat::Sevenz => write!(f, "7z"),
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", format!("error: {}", err).red().bold());
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let cli = Cli::parse();
    let theme = ColorfulTheme::default();

    match cli.command {
        None => {
            print_help_hint();
        }

        Some(Commands::New {
            name,
            r#type,
            template,
            yes,
        }) => {
            cmd_new(&theme, name, r#type, template, yes)?;
        }

        Some(Commands::Add { name, path, part }) => {
            cmd_add(name, path, part)?;
        }

        Some(Commands::List { all, r#type }) => {
            cmd_list(all, r#type)?;
        }

        Some(Commands::Find { query, verbose }) => {
            cmd_find(query, verbose)?;
        }

        Some(Commands::Glob { pattern, path }) => {
            cmd_glob(pattern, path)?;
        }

        Some(Commands::Grep { pattern, path }) => {
            cmd_grep(pattern, path)?;
        }

        Some(Commands::Archive {
            name,
            path,
            format,
            output,
            yes,
        }) => {
            cmd_archive(&theme, name, path, format, output, yes)?;
        }

        Some(Commands::Clean { yes }) => {
            cmd_clean(&theme, yes)?;
        }

        Some(Commands::Doctor) => {
            cmd_doctor()?;
        }

        Some(Commands::System { action }) => match action {
            SystemCmd::Status => cmd_system_status()?,
            SystemCmd::Clean { yes } => cmd_system_clean(yes)?,
        },

        Some(Commands::Vendor { action }) => {
            cmd_vendor(action)?;
        }

        Some(Commands::Git {
            status,
            add,
            commit,
            push,
            pull,
            log,
        }) => {
            cmd_git(status, add, commit, push, pull, log)?;
        }

        Some(Commands::Tools) => {
            cmd_tools()?;
        }

        Some(Commands::Set { name, r#type, path }) => {
            cmd_set(name, r#type, path)?;
        }

        Some(Commands::Check) => {
            cmd_check()?;
        }

        Some(Commands::Size { top, path }) => {
            cmd_size(top, path)?;
        }

        Some(Commands::Duplicate { path, action }) => {
            cmd_duplicate(path, action)?;
        }

        Some(Commands::Dedup { path, yes }) => {
            cmd_dedup(path, yes)?;
        }

        Some(Commands::Benchmark) => {
            cmd_benchmark()?;
        }

        Some(Commands::Templates) => {
            cmd_templates()?;
        }
        Some(Commands::Ignore { list, add, remove }) => {
            cmd_ignore(list, add, remove)?;
        }

        Some(Commands::Completion { shell, output }) => {
            print_completion(shell, output)?;
        }
    }

    Ok(())
}

fn print_help_hint() {
    println!("{}", LOGO.cyan().bold());
    println!("{}", "filz workspace manager   v2.0.0".dimmed());
    println!();

    let reg = match load_registry() {
        Ok(r) => r,
        Err(_) => Registry::default(),
    };

    if !reg.items.is_empty() {
        let total_items = reg.items.len();
        let total_size: u64 = reg
            .items
            .values()
            .map(|it| dir_size_bytes(Path::new(&it.path)))
            .sum();
        println!(
            "{}",
            format!(
                "[workspace]  {}  {} projects • {:.1} MB",
                workspace_root().display(),
                total_items,
                bytes_to_mb(total_size)
            )
            .dimmed()
        );
        println!();

        let active: Vec<_> = reg
            .items
            .values()
            .filter(|it| !it.category_path.contains("archive"))
            .collect();
        let archived: Vec<_> = reg
            .items
            .values()
            .filter(|it| it.category_path.contains("archive"))
            .collect();

        if !active.is_empty() {
            println!("{}", "ACTIVE".bold());
            for it in active.iter().take(5) {
                let size_mb = bytes_to_mb(dir_size_bytes(Path::new(&it.path)));
                let date = it.created_at.split('T').next().unwrap_or("");
                println!(
                    "  {}  [{}:{}]  {:.1} MB  {}",
                    it.name.bold(),
                    it.type_,
                    it.stack.dimmed(),
                    size_mb,
                    date.dimmed()
                );
            }
            if active.len() > 5 {
                println!(
                    "  {} {}",
                    "...".dimmed(),
                    format!("+{} more", active.len() - 5).dimmed()
                );
            }
            println!();
        }

        if !archived.is_empty() {
            println!("{} {} items", "ARCHIVE".bold(), archived.len());
            for it in archived.iter().take(5) {
                let size_mb = bytes_to_mb(dir_size_bytes(Path::new(&it.path)));
                println!("  {}  {:.1} MB", it.name.dimmed(), size_mb);
            }
            if archived.len() > 5 {
                println!(
                    "  {} {}",
                    "...".dimmed(),
                    format!("+{} more", archived.len() - 5).dimmed()
                );
            }
            println!();
        }
    }

    println!("{}", "QUICK START".bold());
    println!(
        "  {} {}",
        "filz new".green().bold(),
        "  scaffold a project (interactive wizard)"
    );
    println!(
        "  {} {}",
        "filz new --template next".green().bold(),
        "  skip prompts, pick a template by name"
    );
    println!(
        "  {} {}",
        "filz new --name api --type work --yes".green().bold(),
        "  create without prompts"
    );
    println!(
        "  {} {}",
        "filz list".green().bold(),
        "  see what you created"
    );
    println!(
        "  {} {}",
        "filz find <name>".green().bold(),
        "  search the registry"
    );
    println!(
        "  {} {}",
        "filz glob \"*.ts\"".green().bold(),
        "  find files by name"
    );
    println!(
        "  {} {}",
        "filz grep \"TODO\"".green().bold(),
        "  search file contents"
    );
    println!(
        "  {} {}",
        "filz clean".green().bold(),
        "  reclaim disk (preview first)"
    );
    println!();
    println!("{}", "PRIMARY                    ADVANCED".bold());
    println!("  new  add  list  find      vendor  benchmark  dedup  system");
    println!("  glob grep archive clean   doctor  tools  check  ignore");
    println!("                               git  templates  completion");
    println!();
    println!("{}", "Aliases: ls=list, f=find, g=glob, gr=grep".dimmed());
    println!("  {} {}", "filz ls".cyan(), "  same as 'filz list'");
    println!(
        "  {} {}",
        "filz gr README".cyan(),
        "  same as 'filz grep README'"
    );
    println!();
    println!("{}", "Run 'filz <command> --help' for details.".dimmed());
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn workspace_root() -> PathBuf {
    config::workspace_root()
}

fn ensure_dir(p: &Path) -> AppResult<()> {
    fs::create_dir_all(p)?;
    Ok(())
}

fn sanitize_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Message("ชื่อว่างไม่ได้".into()));
    }

    let bad = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if trimmed.chars().any(|c| bad.contains(&c)) {
        return Err(AppError::Message("ชื่อมีอักขระต้องห้ามสำหรับ path".into()));
    }

    Ok(trimmed.to_string())
}

fn legacy_registry_path() -> PathBuf {
    config::legacy_registry_path()
}

fn load_registry() -> AppResult<Registry> {
    let pb_path = config::registry_path();
    if pb_path.exists() {
        let bytes = fs::read(&pb_path)?;
        let pb_reg = pb::Registry::decode(&bytes[..])
            .map_err(|e| AppError::Message(format!("proto decode error: {}", e)))?;
        return Ok(Registry::from_proto(pb_reg));
    }

    // Legacy JSON fallback
    let candidates = [legacy_registry_path(), PathBuf::from(".registry.json")];

    for p in candidates {
        if p.exists() {
            let s = fs::read_to_string(&p)?;
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(obj) = v.get("projects").and_then(|x| x.as_object()) {
                    let mut items = HashMap::new();
                    for (k, val) in obj {
                        if let Some(name) = val.get("name").and_then(|x| x.as_str()) {
                            let type_ = val
                                .get("type")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let stack = val
                                .get("stack")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let category_path = val
                                .get("category_path")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let path = val
                                .get("path")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let created_at = val
                                .get("created_at")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            items.insert(
                                k.clone(),
                                Item {
                                    name: name.to_string(),
                                    type_,
                                    stack,
                                    category_path,
                                    path,
                                    created_at,
                                },
                            );
                        }
                    }
                    if !items.is_empty() {
                        return Ok(Registry { items });
                    }
                }
            }
            // Also try flat JSON format
            if let Ok(mut reg) = serde_json::from_str::<FlatRegistry>(&s) {
                let items: HashMap<String, Item> = reg
                    .items
                    .drain()
                    .map(|(k, v)| {
                        (
                            k,
                            Item {
                                name: v.name,
                                type_: v.type_,
                                stack: v.stack,
                                category_path: v.category_path,
                                path: v.path,
                                created_at: v.created_at,
                            },
                        )
                    })
                    .collect();
                if !items.is_empty() {
                    return Ok(Registry { items });
                }
            }
        }
    }

    Ok(Registry::default())
}

#[derive(serde::Deserialize)]
struct FlatRegistry {
    #[serde(default)]
    items: HashMap<String, FlatItem>,
}

#[derive(serde::Deserialize)]
struct FlatItem {
    name: String,
    #[serde(rename = "type")]
    type_: String,
    stack: String,
    category_path: String,
    path: String,
    created_at: String,
}

fn save_registry(reg: &Registry) -> AppResult<()> {
    let pb = reg.to_proto();
    let mut bytes = Vec::new();
    pb.encode(&mut bytes)
        .map_err(|e| AppError::Message(format!("proto encode error: {}", e)))?;
    let p = config::registry_path();
    if let Some(parent) = p.parent() {
        ensure_dir(parent)?;
    }
    fs::write(&p, &bytes)?;
    Ok(())
}

fn stacks_for_type(t: &str) -> Vec<&'static str> {
    match t {
        "work" | "personal" | "client" | "lab" => vec![
            "rust", "node", "next", "react", "python", "go", "tauri", "tool",
        ],
        "asset" => vec![
            "image-pack",
            "icon-set",
            "3d-pack",
            "video-project",
            "audio-pack",
            "figma",
            "blender",
            "texture-pack",
            "font-pack",
            "template-pack",
        ],
        "doc" => vec![
            "contract",
            "invoice",
            "note",
            "knowledge-base",
            "portfolio",
            "reference",
        ],
        "archive" => vec!["archive"],
        "tool" => vec!["rust", "node", "python", "go"],
        _ => vec!["rust", "node", "next", "python", "go"],
    }
}

const VALID_TYPES: &[&str] = &[
    "work", "personal", "client", "lab", "asset", "doc", "archive", "tool",
];

fn resolve_path(type_: &str, stack: &str, name: &str) -> (PathBuf, String) {
    let root = workspace_root();

    match type_ {
        "work" => (
            root.join("01-projects").join("01-work").join(name),
            "01-projects/01-work".into(),
        ),
        "personal" => (
            root.join("01-projects").join("02-personal").join(name),
            "01-projects/02-personal".into(),
        ),
        "client" => (
            root.join("01-projects").join("03-clients").join(name),
            "01-projects/03-clients".into(),
        ),
        "lab" => (
            root.join("01-projects").join("99-lab").join(name),
            "01-projects/99-lab".into(),
        ),
        "tool" => (root.join("tools").join(name), "tools".into()),
        "asset" => {
            let sub = match stack {
                "image-pack" => "01-images",
                "icon-set" => "02-icons",
                "3d-pack" | "blender" => "03-3d",
                "video-project" => "04-video",
                "audio-pack" => "05-audio",
                "figma" => "06-design",
                "font-pack" => "07-fonts",
                "texture-pack" => "08-textures",
                "template-pack" => "09-templates",
                _ => "01-images",
            };
            (
                root.join("02-assets").join(sub).join(name),
                format!("02-assets/{}", sub),
            )
        }
        "doc" => {
            let sub = match stack {
                "contract" => "01-contracts",
                "invoice" => "02-invoices",
                "note" => "03-notes",
                "reference" => "04-refs",
                "knowledge-base" => "05-knowledge",
                "portfolio" | "resume" => "06-resume",
                _ => "03-notes",
            };
            (
                root.join("03-docs").join(sub).join(name),
                format!("03-docs/{}", sub),
            )
        }
        "archive" => (root.join("04-archive").join(name), "04-archive".into()),
        _ => (
            root.join("01-projects").join("01-work").join(name),
            "01-projects/01-work".into(),
        ),
    }
}

fn dir_size_bytes(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut size = 0u64;
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                size += meta.len();
            }
        }
    }
    size
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

#[allow(dead_code)] // wired to Tree subcommand when needed
fn print_tree(path: &Path, max_depth: usize) {
    if path.is_file() {
        println!("{}", path.display());
        return;
    }
    println!(
        "{}/",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    let mut stack: Vec<(PathBuf, String)> = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        dirs.sort_by_key(|e| e.file_name());
        for (i, entry) in dirs.iter().enumerate() {
            let is_last = i == dirs.len() - 1;
            let prefix = if is_last { "└── " } else { "├── " };
            let name = entry.file_name();
            let display = name.to_string_lossy();
            if entry.path().is_dir() {
                println!("{}{}", prefix, display);
                let child_prefix = if is_last { "    " } else { "│   " };
                stack.push((entry.path(), child_prefix.to_string()));
            } else {
                println!("{}{}", prefix, display);
            }
        }
    }
    while let Some((dir, parent_prefix)) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            dirs.sort_by_key(|e| e.file_name());
            for (i, entry) in dirs.iter().enumerate() {
                let is_last = i == dirs.len() - 1;
                let prefix = if is_last { "└── " } else { "├── " };
                let child_prefix = if is_last { "    " } else { "│   " };
                let name = entry.file_name();
                let display = name.to_string_lossy();
                print!("{}{}", parent_prefix, prefix);
                if entry.path().is_dir() {
                    println!("{}/", display);
                    let new_prefix = format!("{}{}", parent_prefix, child_prefix);
                    let depth = parent_prefix.matches('│').count() + 1;
                    if depth < max_depth {
                        stack.push((entry.path(), new_prefix));
                    }
                } else {
                    println!("{}", display);
                }
            }
        }
    }
}

fn color_for_type(t: &str) -> colored::ColoredString {
    match t {
        "work" => t.blue(),
        "personal" => t.magenta(),
        "client" => t.yellow(),
        "lab" => t.bright_magenta(),
        "asset" => t.green(),
        "doc" => t.cyan(),
        "archive" => t.white(),
        "tool" => t.bright_blue(),
        _ => t.normal(),
    }
}

fn create_common_files(path: &Path, name: &str, type_: &str, stack: &str) -> AppResult<()> {
    ensure_dir(path)?;

    let gitignore = r#"# System Junk
Thumbs.db
desktop.ini
.DS_Store

# Deps / Build
node_modules/
.next/
dist/
build/
.turbo/
target/
__pycache__/
.venv/
.pytest_cache/
vendor/
bin/
.vercel/

# Env
.env
.env.local

# Logs
logs/
npm-debug.log
*.log
"#;

    fs::write(path.join(".gitignore"), gitignore)?;

    let readme = format!(
        "# {}\n\n- TYPE: {}\n- STACK: {}\n- PATH: {}\n- CREATED: {}\n",
        name,
        type_,
        stack,
        path.display(),
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    );
    fs::write(path.join("README.md"), readme)?;

    Ok(())
}

fn create_rust_project(path: &Path, name: &str) -> AppResult<()> {
    fs::write(
        path.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
            name
        ),
    )?;

    let src = path.join("src");
    ensure_dir(&src)?;
    fs::write(
        src.join("main.rs"),
        "fn main() {\n    println!(\"Hello from filz manager\");\n}\n",
    )?;
    Ok(())
}

fn create_node_project(path: &Path, name: &str, stack: &str) -> AppResult<()> {
    let script_dev = if stack == "next" { "next dev" } else { "vite" };
    let script_build = if stack == "next" {
        "next build"
    } else {
        "vite build"
    };

    let pkg = serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "private": true,
        "type": "module",
        "scripts": {
            "dev": script_dev,
            "build": script_build
        }
    });

    fs::write(
        path.join("package.json"),
        serde_json::to_string_pretty(&pkg)?,
    )?;

    if stack == "next" {
        fs::write(
            path.join("next.config.js"),
            "/** @type {import('next').NextConfig} */\nconst nextConfig = {};\nmodule.exports = nextConfig;\n",
        )?;
    }

    if stack == "tauri" {
        ensure_dir(&path.join("src-tauri"))?;
    }

    Ok(())
}

fn create_python_project(path: &Path, _name: &str) -> AppResult<()> {
    fs::write(path.join("requirements.txt"), "# add deps\n")?;
    let src = path.join("src");
    ensure_dir(&src)?;
    fs::write(
        src.join("main.py"),
        "if __name__ == \"__main__\":\n    print(\"hello from E: Drive Manager\")\n",
    )?;
    Ok(())
}

fn create_go_project(path: &Path, name: &str) -> AppResult<()> {
    fs::write(path.join("go.mod"), format!("module {}\n\ngo 1.22\n", name))?;
    fs::write(
        path.join("main.go"),
        "package main\nimport \"fmt\"\nfunc main(){ fmt.Println(\"hello\") }\n",
    )?;
    Ok(())
}

fn create_template(type_: &str, stack: &str, name: &str, path: &Path) -> AppResult<()> {
    create_common_files(path, name, type_, stack)?;

    match stack {
        "rust" => create_rust_project(path, name)?,
        "node" | "react" | "next" | "tauri" => create_node_project(path, name, stack)?,
        "python" => create_python_project(path, name)?,
        "go" => create_go_project(path, name)?,
        _ => {
            let meta = serde_json::json!({
                "name": name,
                "type": type_,
                "stack": stack,
                "created": now_rfc3339(),
                "tags": []
            });
            fs::write(path.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

            if type_ == "asset" {
                ensure_dir(&path.join("source"))?;
                ensure_dir(&path.join("export"))?;
            }

            if type_ == "doc" {
                ensure_dir(&path.join("files"))?;
            }
        }
    }

    Ok(())
}

#[derive(serde::Deserialize, Debug)]
struct ProjectTemplate {
    name: String,
    description: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    stack: String,
    #[serde(default)]
    folders: Vec<String>,
    #[serde(default)]
    files: Vec<TemplateFile>,
}

#[derive(serde::Deserialize, Debug)]
struct TemplateFile {
    path: String,
    #[serde(default)]
    content: String,
}

fn load_templates() -> Vec<ProjectTemplate> {
    let tmpl_dir = std::env::current_dir()
        .ok()
        .map(|d| d.join("templates"))
        .filter(|p| p.is_dir());

    let tmpl_dir = match tmpl_dir {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut templates = Vec::new();
    if let Ok(entries) = fs::read_dir(&tmpl_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(tmpl) = serde_yaml::from_str::<ProjectTemplate>(&content) {
                        templates.push(tmpl);
                    }
                }
            }
        }
    }
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    templates
}

fn render_template_value(value: &str, name: &str, tmpl: &ProjectTemplate, now: &str) -> String {
    let replacements: Vec<(&str, String)> = vec![
        ("name", name.to_string()),
        ("type", tmpl.r#type.clone()),
        ("stack", tmpl.stack.clone()),
        ("created", now.to_string()),
    ];

    let mut result = value.to_string();
    for (key, val) in &replacements {
        result = result.replace(&format!("{{{{{}}}}}", key), val);
        result = result.replace(&format!("{{{{{}}}}}", key.to_uppercase()), val);
        result = result.replace(&key.to_uppercase(), val);
    }
    result
}

fn apply_template(tmpl: &ProjectTemplate, name: &str, path: &Path) -> AppResult<()> {
    ensure_dir(path)?;

    let now = now_rfc3339();
    for folder in &tmpl.folders {
        let rendered_folder = render_template_value(folder, name, tmpl, &now);
        let folder_path = path.join(rendered_folder);
        ensure_dir(&folder_path)?;
    }

    for file in &tmpl.files {
        let rendered_path = render_template_value(&file.path, name, tmpl, &now);
        let file_path = path.join(&rendered_path);
        if let Some(parent) = file_path.parent() {
            ensure_dir(parent)?;
        }

        let mut content = file.content.clone();
        let replacements: Vec<(&str, String)> = vec![
            ("name", name.to_string()),
            ("type", tmpl.r#type.clone()),
            ("stack", tmpl.stack.clone()),
            ("created", now.clone()),
        ];

        for (key, val) in &replacements {
            content = content.replace(&format!("{{{{{}}}}}", key), val);
            content = content.replace(&format!("{{{{{}}}}}", key.to_uppercase()), val);
            content = content.replace(&key.to_uppercase(), val);
        }
        fs::write(&file_path, content)?;
    }

    Ok(())
}

fn cmd_new(
    theme: &ColorfulTheme,
    name: Option<String>,
    r#type: Option<String>,
    template: Option<String>,
    yes: bool,
) -> AppResult<()> {
    let templates = load_templates();
    let types = VALID_TYPES;

    // ── Step 1: Choose type ──
    let final_type = if let Some(t) = r#type {
        t.to_lowercase()
    } else {
        let idx = Select::with_theme(theme)
            .with_prompt("ขั้นตอนที่ 1: เลือกประเภทงาน")
            .items(types.iter().copied())
            .default(0)
            .interact()?;
        types[idx].to_string()
    };

    // ── Step 2: Choose template or stack ──
    let (final_stack, template_used) = if let Some(tmpl_name) = template {
        // User specified a template name directly
        if let Some(tmpl) = templates
            .iter()
            .find(|t| t.name == tmpl_name.to_lowercase())
        {
            (tmpl.stack.clone(), true)
        } else {
            // Treat as stack name if no matching template
            (tmpl_name.to_lowercase(), false)
        }
    } else if let Some(stack) = template.or_else(|| {
        // Try to match template by name if user passed --template
        None
    }) {
        (stack, true)
    } else {
        // Interactive template selection
        let tmpl_names: Vec<String> = templates.iter().map(|t| t.name.clone()).collect();
        let stack_options = stacks_for_type(&final_type);

        let choices: Vec<String> = if !tmpl_names.is_empty() {
            let mut choices = vec!["(ใช้ template จากรายการด้านล่าง)".to_string()];
            choices.extend(tmpl_names.clone());
            choices.extend(stack_options.iter().map(|s| format!("(stack) {}", s)));
            choices
        } else {
            stack_options.iter().map(|s| s.to_string()).collect()
        };

        let idx = Select::with_theme(theme)
            .with_prompt(format!(
                "ขั้นตอนที่ 2: เลือก template หรือ stack สำหรับ [{}]",
                final_type
            ))
            .items(choices.iter().cloned())
            .default(0)
            .interact()?;

        if idx == 0 && !tmpl_names.is_empty() {
            // User chose to pick from templates - show them
            let tmpl_idx = Select::with_theme(theme)
                .with_prompt("เลือก template")
                .items(tmpl_names.iter().cloned())
                .default(0)
                .interact()?;
            let tmpl = &templates[tmpl_idx];
            (tmpl.stack.clone(), true)
        } else {
            let selected = &choices[idx];
            let stack = selected.strip_prefix("(stack) ").unwrap_or(selected);
            (stack.to_string(), false)
        }
    };

    // ── Step 3: Get name and preview path ──
    let final_name = if let Some(n) = name {
        sanitize_name(&n)?
    } else {
        let input: String = Input::with_theme(theme)
            .with_prompt("ขั้นตอนที่ 3: ตั้งชื่อรายการ")
            .interact_text()?;
        sanitize_name(&input)?
    };

    let (proj_path, category) = resolve_path(&final_type, &final_stack, &final_name);

    // Preview
    println!();
    println!("{}", "╔══════════════════════════════════════╗".cyan());
    println!(
        "{}",
        "║       สรุปการสร้างรายการ                ║".cyan().bold()
    );
    println!("{}", "╠══════════════════════════════════════╣".cyan());
    println!("{}  {}", "  ชื่อ:".bold(), final_name);
    println!("{}  {}", "  ประเภท:", color_for_type(&final_type));
    println!("{}  {}", "  Stack:", final_stack.dimmed());
    if template_used {
        println!(
            "{}  {}",
            "  Template:".green(),
            "ใช้ template จาก YAML".green()
        );
    }
    println!(
        "{}  {}",
        "  ที่เก็บ:".dimmed(),
        format!("{}", proj_path.display()).dimmed()
    );
    println!("{}  {}", "  หมวด:", category.dimmed());
    println!("{}", "╚══════════════════════════════════════╝".cyan());
    println!();

    if proj_path.exists() {
        return Err(AppError::Message(format!(
            "มีรายการอยู่แล้วที่ path นี้: {}",
            proj_path.display()
        )));
    }

    // ── Step 4: Confirm ──
    let confirmed = if yes {
        true
    } else {
        Confirm::with_theme(theme)
            .with_prompt("ยืนยันการสร้าง?")
            .default(true)
            .interact()?
    };

    if !confirmed {
        println!("{}", "ยกเลิกแล้ว".dimmed());
        return Ok(());
    }

    // Apply template if found, else use built-in
    if template_used {
        if let Some(tmpl) = templates.iter().find(|t| t.stack == final_stack) {
            apply_template(tmpl, &final_name, &proj_path)?;
        } else {
            create_template(&final_type, &final_stack, &final_name, &proj_path)?;
        }
    } else {
        create_template(&final_type, &final_stack, &final_name, &proj_path)?;
    }

    let mut reg = load_registry()?;
    reg.items.insert(
        final_name.clone(),
        Item {
            name: final_name.clone(),
            type_: final_type.clone(),
            stack: final_stack.clone(),
            category_path: category,
            path: proj_path.to_string_lossy().to_string(),
            created_at: now_rfc3339(),
        },
    );
    save_registry(&reg)?;

    let cache_root = config::workspace_root();
    let behavior_path = BehaviorProfile::path(&cache_root);
    let mut behavior = BehaviorProfile::load(&behavior_path);
    behavior.observe(&final_type, &final_stack);
    let _ = behavior.save(&behavior_path);

    println!(
        "{}",
        format!("✓ สร้าง {} → {}", final_name, proj_path.display())
            .green()
            .bold()
    );

    Ok(())
}

fn cmd_add(name: String, path: Option<PathBuf>, part: Option<String>) -> AppResult<()> {
    let name = sanitize_name(&name)?;

    // If --part, add subfolder to existing item
    if let Some(folder) = part {
        let reg = load_registry()?;
        let item = reg
            .items
            .get(&name)
            .ok_or_else(|| AppError::Message(format!("ไม่พบ item: {}", name)))?;
        let base = PathBuf::from(&item.path);
        let sub = base.join(&folder);
        ensure_dir(&sub)?;
        println!(
            "{}",
            format!("✓ เพิ่ม {}/{} สำเร็จ", name, folder).green().bold()
        );
        return Ok(());
    }

    let final_path = if let Some(p) = path {
        p
    } else {
        return Err(AppError::Message(
            "ต้องระบุ --path หรือใช้ --part กับรายการที่มีอยู่".into(),
        ));
    };

    let mut reg = load_registry()?;
    let item = reg.items.get(&name).cloned();
    if let Some(existing) = item {
        let mut updated = existing;
        updated.path = final_path.to_string_lossy().to_string();
        reg.items.insert(name.clone(), updated);
    } else {
        return Err(AppError::Message(format!(
            "ไม่พบ item: {} ใน registry ใช้ 'filz new' สร้างก่อน",
            name
        )));
    }
    save_registry(&reg)?;

    println!(
        "{}",
        format!("✓ เพิ่ม path ให้ {} -> {}", name, final_path.display())
            .green()
            .bold()
    );
    Ok(())
}

fn cmd_benchmark() -> AppResult<()> {
    println!("{}", header("Diagnostic: Performance Benchmark"));
    println!(
        "{}",
        "(เครื่องมือนี้สำหรับวัดประสิทธิภาพเท่านั้น ไม่เกี่ยวกับงานหลัก)".dimmed()
    );
    println!();

    use std::time::Instant;

    let bench_dir = std::env::temp_dir().join("file_cli_bench");
    let _ = std::fs::remove_dir_all(&bench_dir);
    std::fs::create_dir_all(&bench_dir).unwrap();

    // --- Create test data: 10K files, ~100MB total ---
    let total_size = with_spinner("creating test data", || {
        let t_create = Instant::now();
        for i in 0..10_000 {
            let dir = bench_dir.join(format!("dir-{:04}", i / 100));
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join(format!("file-{:04}.txt", i));
            let content = format!("line1\nTARGET_LINE_{}\n{}\n", i, "x".repeat(10000));
            std::fs::write(&file, &content).unwrap();
        }
        let create_t = t_create.elapsed();
        let total_size: u64 = std::fs::read_dir(&bench_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .flat_map(|e| {
                std::fs::read_dir(e.path())
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
            })
            .filter_map(|e| std::fs::metadata(e.path()).ok().map(|m| m.len()))
            .sum();
        println!(
            "  {} files, {:.1} MB total, created in {:?}",
            "10,000".bold(),
            total_size as f64 / (1024.0 * 1024.0),
            create_t
        );
        Ok(total_size)
    })?;
    println!();

    // --- Test 1: Content Search (grep) ---
    println!("{}", header("Test 1: Content Search"));
    let pattern = "TARGET_LINE_5000";

    // Our grep (uses rg)
    let our_grep_t = {
        let t0 = Instant::now();
        let rg_path = config::vendor_bin("rg");
        if let Some(rg) = rg_path {
            let _ = std::process::Command::new(&rg)
                .args(["--color=never", "-n", pattern, &bench_dir.to_string_lossy()])
                .output();
        }
        t0.elapsed()
    };

    // GNU find + grep
    let find_grep_t = {
        let t0 = Instant::now();
        let _ = std::process::Command::new("find")
            .args([
                &bench_dir.to_string_lossy(),
                "-type",
                "f",
                "-exec",
                "grep",
                "-l",
                pattern,
                "{}",
                "+",
            ])
            .output();
        t0.elapsed()
    };

    // Raw rg (system)
    let raw_rg_t = {
        let t0 = Instant::now();
        let _ = std::process::Command::new("rg")
            .args(["--color=never", pattern, &bench_dir.to_string_lossy()])
            .output();
        t0.elapsed()
    };

    // Pure Rust fallback
    let rust_grep_t = {
        let t0 = Instant::now();
        let mut count = 0usize;
        fn walk_search(dir: &std::path::Path, pattern: &str, count: &mut usize) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        walk_search(&p, pattern, count);
                    } else if let Ok(content) = std::fs::read_to_string(&p) {
                        if content.contains(pattern) {
                            *count += 1;
                        }
                    }
                }
            }
        }
        walk_search(&bench_dir, pattern, &mut count);
        t0.elapsed()
    };

    print_comparison(&[
        ("filz grep (rg-based)", our_grep_t),
        ("rg (raw)", raw_rg_t),
        ("find + grep", find_grep_t),
        ("pure Rust fallback", rust_grep_t),
    ]);

    println!();

    // --- Test 2: File Listing (glob) ---
    println!("{}", header("Test 2: File Listing"));

    // Our glob (pure Rust walk)
    let our_glob_t = {
        let t0 = Instant::now();
        let mut count = 0usize;
        fn walk_glob(dir: &std::path::Path, count: &mut usize) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        walk_glob(&p, count);
                    } else if p.extension().and_then(|e| e.to_str()) == Some("txt") {
                        *count += 1;
                    }
                }
            }
        }
        walk_glob(&bench_dir, &mut count);
        t0.elapsed()
    };

    // GNU find -name
    let find_name_t = {
        let t0 = Instant::now();
        let _ = std::process::Command::new("find")
            .args([&bench_dir.to_string_lossy(), "-name", "*.txt", "-type", "f"])
            .output();
        t0.elapsed()
    };

    // ls -R
    let ls_r_t = {
        let t0 = Instant::now();
        let _ = std::process::Command::new("ls")
            .args(["-R", &bench_dir.to_string_lossy()])
            .output();
        t0.elapsed()
    };

    print_comparison(&[
        ("filz glob (Rust)", our_glob_t),
        ("find -name", find_name_t),
        ("ls -R", ls_r_t),
    ]);

    println!();

    // --- Test 3: Registry Search vs OS Search ---
    println!("{}", header("Test 3: Registry Search"));

    // Build registry with 10K items
    let mut items = HashMap::new();
    for i in 0..10_000 {
        let name = format!("project-{:04}", i);
        let type_ = match i % 5 {
            0 => "work",
            1 => "doc",
            2 => "asset",
            3 => "personal",
            _ => "lab",
        };
        items.insert(
            name.clone(),
            Item {
                name,
                type_: type_.into(),
                stack: "rust".into(),
                category_path: format!("01-projects/01-{}", type_),
                path: format!("/data/projects/{:04}", i),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
        );
    }
    let reg = Registry { items };

    // Save to temp file
    let pb = reg.to_proto();
    let mut buf = Vec::new();
    pb.encode(&mut buf).unwrap();
    let reg_path = bench_dir.join("registry.pb");
    std::fs::write(&reg_path, &buf).unwrap();

    // Our registry search (load + HashMap lookup)
    let our_reg_t = {
        let t0 = Instant::now();
        let bytes = std::fs::read(&reg_path).unwrap();
        let pb_reg = pb::Registry::decode(&bytes[..]).unwrap();
        let decoded = Registry::from_proto(pb_reg);
        for i in 0..1000 {
            let key = format!("project-{:04}", i % 10000);
            let _ = decoded.items.get(&key);
        }
        t0.elapsed()
    };

    // OS find + grep (simulated: search 1000 names in file)
    let os_search_t = {
        let t0 = Instant::now();
        let content = std::fs::read_to_string(&reg_path).unwrap_or_default();
        for i in 0..1000 {
            let target = format!("project-{:04}", i % 10000);
            let _ = content.contains(&target);
        }
        t0.elapsed()
    };

    let speedup = os_search_t.as_secs_f64() / our_reg_t.as_secs_f64();
    println!("  Registry (10K items, 1000 lookups):");
    println!("    filz find (protobuf+HashMap): {:>8.2?}", our_reg_t);
    println!("    OS brute-force:             {:>8.2?}", os_search_t);
    println!("    Speedup:                    {:>8.1}x", speedup);
    println!();

    // --- Test 4: Registry Scale ---
    println!("{}", header("Test 4: Registry Scale"));
    for &n in &[1_000, 10_000, 100_000, 1_000_000] {
        let mut items = HashMap::new();
        for i in 0..n {
            let name = format!("item-{:07}", i);
            items.insert(
                name.clone(),
                Item {
                    name,
                    type_: "work".into(),
                    stack: "rust".into(),
                    category_path: "01-projects/01-work".into(),
                    path: format!("/data/projects/{:07}", i),
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
            );
        }
        let reg = Registry { items };
        let pb = reg.to_proto();
        let mut b = Vec::new();
        pb.encode(&mut b).unwrap();
        let file_size = b.len();

        let t0 = Instant::now();
        let pb_reg = pb::Registry::decode(&b[..]).unwrap();
        let decoded = Registry::from_proto(pb_reg);
        let decode_t = t0.elapsed();

        let t1 = Instant::now();
        for i in 0..1000 {
            let key = format!("item-{:07}", i % n);
            let _ = decoded.items.get(&key);
        }
        let search_t = t1.elapsed();

        let size_str = if file_size >= 1024 * 1024 {
            format!("{} MB", file_size / (1024 * 1024))
        } else {
            format!("{} KB", file_size / 1024)
        };
        println!(
            "  {:>8} items | {:>6} | decode {:>7.2?} | search {:>7.2?} per op",
            n,
            size_str,
            decode_t,
            search_t / 1000
        );
    }

    println!();

    // --- Save results to JSON ---
    let results = serde_json::json!({
        "timestamp": now_rfc3339(),
        "device": format!("{} {}", detect_os(), detect_arch()),
        "tests": {
            "content_search": {
                "our_grep_ms": our_grep_t.as_millis(),
                "raw_rg_ms": raw_rg_t.as_millis(),
                "find_grep_ms": find_grep_t.as_millis(),
                "pure_rust_ms": rust_grep_t.as_millis(),
            },
            "file_listing": {
                "our_glob_ms": our_glob_t.as_millis(),
                "find_name_ms": find_name_t.as_millis(),
                "ls_r_ms": ls_r_t.as_millis(),
            },
            "registry_search": {
                "our_ms": our_reg_t.as_millis(),
                "os_bruteforce_ms": os_search_t.as_millis(),
                "speedup": format!("{:.1}x", speedup),
            },
        },
        "test_data": {
            "files": 10000,
            "total_size_mb": format!("{:.1}", total_size as f64 / (1024.0 * 1024.0)),
        }
    });

    let results_path = bench_dir.join("benchmark_results.json");
    std::fs::write(
        &results_path,
        serde_json::to_string_pretty(&results).unwrap(),
    )
    .unwrap();
    println!(
        "{} {}",
        "✓ Results saved to:".green(),
        results_path.display()
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&bench_dir);

    println!();
    println!("{}", "✓ Benchmark เสร็จสิ้น".green().bold());
    Ok(())
}

fn print_comparison(results: &[(&str, std::time::Duration)]) {
    let fastest = results
        .iter()
        .map(|(_, d)| *d)
        .min()
        .unwrap_or(std::time::Duration::ZERO);
    for &(name, dur) in results {
        let factor = dur.as_secs_f64() / fastest.as_secs_f64();
        let bar_len = (factor * 20.0).min(60.0) as usize;
        let bar: String = std::iter::repeat('#').take(bar_len).collect();
        let label = if factor <= 1.0 {
            format!("{} (fastest)", name.green().bold())
        } else {
            format!("{} ({:.1}x)", name, factor)
        };
        println!("  {:>24} {:>8.2?}  {}", label, dur, bar.dimmed());
    }
}

fn colorize_file(name: &str) -> colored::ColoredString {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs" | "py" | "js" | "ts" | "go" | "rb" | "java" | "c" | "cpp" | "h" => name.blue().bold(),
        "html" | "css" | "scss" | "less" => name.magenta(),
        "json" | "yaml" | "yml" | "toml" | "xml" => name.cyan(),
        "md" | "txt" | "rst" => name.white().bold(),
        "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "ico" => name.green(),
        "zip" | "tar" | "gz" | "7z" | "rar" => name.red(),
        "sh" | "bash" | "zsh" | "fish" | "ps1" => name.yellow(),
        "lock" | "sum" | "hash" => name.dimmed(),
        "" => name.bold(),
        _ => name.normal(),
    }
}

fn colorize_dir(name: &str) -> colored::ColoredString {
    name.blue().bold()
}

fn colorize_size(size: u64) -> colored::ColoredString {
    let s = if size >= 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{} B", size)
    };
    if size >= 1024 * 1024 {
        s.red().bold()
    } else if size >= 100 * 1024 {
        s.yellow()
    } else {
        s.green()
    }
}

fn cmd_vendor(action: VendorCmd) -> AppResult<()> {
    let vendor_dir = std::env::current_dir().unwrap_or_default().join("vendor");

    match action {
        VendorCmd::List => {
            let headers = vec!["Name", "Size", "Path"];
            let mut rows = Vec::new();

            if vendor_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&vendor_dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_file() {
                            let name = path.file_name().unwrap().to_string_lossy().to_string();
                            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                            let size_str = if size >= 1024 * 1024 {
                                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
                            } else {
                                format!("{:.1} KB", size as f64 / 1024.0)
                            };
                            rows.push(vec![name, size_str, path.to_string_lossy().to_string()]);
                        }
                    }
                }
            }

            render_banner("Vendor Tools");
            render_table(&headers, &rows);
        }

        VendorCmd::Search { name } => {
            println!("{}", format!("ค้นหา tool: {}", name).cyan().bold());

            // Check vendor dir
            let vendor_path = vendor_dir.join(&name);
            let has_vendor = vendor_path.exists();
            if has_vendor {
                let size = std::fs::metadata(&vendor_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                println!("  {} vendor ({:.1} KB)", "✓".green(), size as f64 / 1024.0);
            }

            // Check system PATH
            let has_system = std::process::Command::new("which")
                .arg(&name)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if has_system {
                let path = std::process::Command::new("which")
                    .arg(&name)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default();
                println!("  {} system ({})", "✓".green(), path);
            }

            // Check markdown docs
            let docs_dir = vendor_dir.join("docs");
            let md_path = docs_dir.join(format!("{}.md", name));
            if md_path.exists() {
                println!("  {} doc", "✓".green());
            }

            // If nothing found, offer to install
            if !has_vendor && !has_system {
                println!("  {} ไม่พบ — กำลังติดตั้ง...", "→".yellow());
                // Try to install via apt
                let status = std::process::Command::new("apt")
                    .args(["install", "-y", &name])
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        println!("  {} ติดตั้ง {} สำเร็จ", "✓".green(), name);
                    }
                    _ => {
                        println!(
                            "  {} ติดตั้งไม่ได้ — ลอง: apt install {} หรือ download ด้วยมือ",
                            "✗".red(),
                            name
                        );
                    }
                }
            }
        }

        VendorCmd::Store { name, url } => {
            let docs_dir = vendor_dir.join("docs");
            std::fs::create_dir_all(&docs_dir)?;

            let md_path = docs_dir.join(format!("{}.md", name));

            // Fetch content from URL
            println!("{}", format!("กำลังดึงข้อมูลจาก {}...", url).dimmed());

            let output = std::process::Command::new("curl")
                .args(["-sL", "--max-time", "10", &url])
                .output()
                .map_err(|e| AppError::Message(format!("curl ไม่ทำงาน: {}", e)))?;

            if !output.status.success() {
                return Err(AppError::Message(format!(
                    "ดาวน์โหลดล้มเหลว (exit: {:?})",
                    output.status.code()
                )));
            }

            let content = String::from_utf8_lossy(&output.stdout);

            // Generate markdown
            let md = format!(
                "# {}

URL: {}
Fetched: {}
Size: {} bytes

## Content

{}
",
                name,
                url,
                now_rfc3339(),
                content.len(),
                content.chars().take(50000).collect::<String>()
            );

            std::fs::write(&md_path, &md)?;

            println!(
                "{}",
                format!("✓ บันทึก {} -> {}", name, md_path.display())
                    .green()
                    .bold()
            );
        }

        VendorCmd::Dedup => {
            println!("{}", header("Vendor Dedup"));

            if !vendor_dir.exists() {
                println!("{}", "vendor/ ไม่มี".yellow());
                return Ok(());
            }

            // Collect all files with their sizes
            let mut files: Vec<(String, u64, String)> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&vendor_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path.file_name().unwrap().to_string_lossy().to_string();
                        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        files.push((name, size, path.to_string_lossy().to_string()));
                    }
                }
            }

            // Group by size (potential duplicates)
            let mut size_map: std::collections::HashMap<u64, Vec<&str>> =
                std::collections::HashMap::new();
            for (name, size, _) in &files {
                size_map.entry(*size).or_default().push(name);
            }

            let headers = vec!["Name", "Size", "Status"];
            let mut rows = Vec::new();

            for (name, size, _) in &files {
                let dupes = size_map.get(&size).map(|v| v.len()).unwrap_or(1);
                let status = if dupes > 1 {
                    "DUP".red().to_string()
                } else {
                    "OK".green().to_string()
                };
                let size_str = if *size >= 1024 * 1024 {
                    format!("{:.1} MB", *size as f64 / (1024.0 * 1024.0))
                } else {
                    format!("{:.1} KB", *size as f64 / 1024.0)
                };
                rows.push(vec![name.clone(), size_str, status]);
            }

            render_table(&headers, &rows);

            let dupes: Vec<_> = size_map.iter().filter(|(_, v)| v.len() > 1).collect();
            if !dupes.is_empty() {
                println!();
                println!("{}", format!("พบ {} กลุ่มที่ซ้ำ", dupes.len()).yellow());
            } else {
                println!();
                println!("{}", "ไม่มีไฟล์ซ้ำ".green());
            }
        }
    }

    Ok(())
}

fn cmd_dedup(path: Option<PathBuf>, yes: bool) -> AppResult<()> {
    use std::collections::HashMap;

    let root =
        path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    println!("{} {}", header("Dedup Scan"), root.display());

    let (_, total_files, dupes) = with_spinner("scanning for duplicates", || {
        let mut size_map: HashMap<u64, Vec<String>> = HashMap::new();
        let mut total_files = 0usize;

        fn walk(dir: &Path, size_map: &mut HashMap<u64, Vec<String>>, total: &mut usize) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    let name = p.file_name().unwrap().to_string_lossy();
                    if should_skip_dir(&name) {
                        continue;
                    }
                    if p.is_dir() {
                        walk(&p, size_map, total);
                    } else if let Ok(meta) = std::fs::metadata(&p) {
                        let size = meta.len();
                        if size > 0 {
                            size_map
                                .entry(size)
                                .or_default()
                                .push(p.to_string_lossy().to_string());
                            *total += 1;
                        }
                    }
                }
            }
        }

        walk(&root, &mut size_map, &mut total_files);

        let mut dupes: Vec<(u64, Vec<String>)> = size_map
            .clone()
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .collect();
        dupes.sort_by(|a, b| b.0.cmp(&a.0));
        Ok((size_map, total_files, dupes))
    })?;

    let headers = vec!["Size", "Count", "Files"];
    let mut rows = Vec::new();
    let mut wasted = 0u64;

    for (size, files) in &dupes {
        let wasted_here = size * (files.len() as u64 - 1);
        wasted += wasted_here;
        let size_str = if *size >= 1024 * 1024 {
            format!("{:.1} MB", *size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} KB", *size as f64 / 1024.0)
        };
        let files_str = files
            .iter()
            .take(2)
            .map(|f| {
                std::path::Path::new(f)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");
        let more = if files.len() > 2 {
            format!(" +{}", files.len() - 2)
        } else {
            String::new()
        };
        rows.push(vec![
            size_str,
            files.len().to_string(),
            format!("{}{}", files_str, more),
        ]);
    }

    render_table(&headers, &rows);

    println!();
    println!(
        "{}",
        format!(
            "Total: {} files | Duplicates: {} groups | Wasted: {:.1} MB",
            total_files,
            dupes.len(),
            wasted as f64 / (1024.0 * 1024.0)
        )
        .yellow()
    );

    if yes {
        let mut deleted = 0usize;
        for (_size, files) in &dupes {
            for f in files.iter().skip(1) {
                let _ = std::fs::remove_file(f);
                deleted += 1;
            }
        }
        println!("{} ลบ {} ไฟล์ซ้ำแล้ว", "✓".green(), deleted);
    } else {
        println!();
        println!("{}", "นี่คือ preview — ใช้ --yes เพื่อลบไฟล์ซ้ำ".dimmed());
    }

    Ok(())
}

fn cmd_size(top: usize, path: Option<PathBuf>) -> AppResult<()> {
    let root =
        path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    println!(
        "{} ไฟล์ขนาดใหญ่สุด {} (top {})",
        "📁".cyan(),
        root.display(),
        top
    );

    let mut files: Vec<(String, u64)> = Vec::new();

    fn walk(dir: &Path, files: &mut Vec<(String, u64)>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                let name = p.file_name().unwrap().to_string_lossy();
                if should_skip_dir(&name) {
                    continue;
                }
                if p.is_dir() {
                    walk(&p, files);
                } else if let Ok(meta) = std::fs::metadata(&p) {
                    let size = meta.len();
                    if size > 0 {
                        files.push((p.to_string_lossy().to_string(), size));
                    }
                }
            }
        }
    }

    walk(&root, &mut files);
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.truncate(top);

    let headers = vec!["#", "Size", "File"];
    let mut rows = Vec::new();
    for (i, (path, size)) in files.iter().enumerate() {
        // Show relative path
        let short = std::path::Path::new(path)
            .strip_prefix(&root)
            .unwrap_or(std::path::Path::new(path))
            .to_string_lossy()
            .to_string();
        rows.push(vec![
            (i + 1).to_string(),
            colorize_size(*size).to_string(),
            short,
        ]);
    }

    render_table(&headers, &rows);

    let total: u64 = files.iter().map(|(_, s)| s).sum();
    println!();
    println!(
        "{}",
        format!(
            "{} files shown, total {:.1} MB",
            files.len(),
            total as f64 / (1024.0 * 1024.0)
        )
        .dimmed()
    );
    Ok(())
}

fn cmd_duplicate(path: Option<PathBuf>, action: Option<String>) -> AppResult<()> {
    let root =
        path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    println!("{} สแกนไฟล์ซ้ำ {} (preview ก่อน)", "🔍".cyan(), root.display());

    let (_, _, dupes) = with_spinner("scanning for duplicate files", || {
        let mut size_map: HashMap<u64, Vec<String>> = HashMap::new();
        let mut total_files = 0usize;

        fn walk(dir: &Path, size_map: &mut HashMap<u64, Vec<String>>, total: &mut usize) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    let name = p.file_name().unwrap().to_string_lossy();
                    if should_skip_dir(&name) {
                        continue;
                    }
                    if p.is_dir() {
                        walk(&p, size_map, total);
                    } else if let Ok(meta) = std::fs::metadata(&p) {
                        let size = meta.len();
                        if size > 0 {
                            size_map
                                .entry(size)
                                .or_default()
                                .push(p.to_string_lossy().to_string());
                            *total += 1;
                        }
                    }
                }
            }
        }

        walk(&root, &mut size_map, &mut total_files);

        let mut dupes: Vec<(u64, Vec<String>)> = size_map
            .clone()
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .collect();
        dupes.sort_by(|a, b| b.0.cmp(&a.0));
        Ok((size_map, total_files, dupes))
    })?;

    if dupes.is_empty() {
        println!("{}", "ไม่มีไฟล์ซ้ำ".green());
        return Ok(());
    }

    let headers = vec!["Size", "Copies", "Files"];
    let mut rows = Vec::new();
    let mut wasted = 0u64;

    for (size, files) in &dupes {
        let wasted_here = *size * (files.len() as u64 - 1);
        wasted += wasted_here;
        let size_str = if *size >= 1024 * 1024 {
            format!("{:.1} MB", *size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} KB", *size as f64 / 1024.0)
        };
        let files_str = files
            .iter()
            .take(2)
            .map(|f| {
                std::path::Path::new(f)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");
        let more = if files.len() > 2 {
            format!(" +{}", files.len() - 2)
        } else {
            String::new()
        };
        rows.push(vec![
            size_str,
            files.len().to_string(),
            format!("{}{}", files_str, more),
        ]);
    }

    render_table(&headers, &rows);

    println!();
    println!(
        "{}",
        format!(
            "{} groups | Wasted: {:.1} MB",
            dupes.len(),
            wasted as f64 / (1024.0 * 1024.0)
        )
        .yellow()
    );

    // If no action specified, show preview only
    if action.is_none() {
        println!();
        println!(
            "{}",
            "นี่คือ preview — ใช้ --action delete|move|copy เพื่อดำเนินการ".dimmed()
        );
        return Ok(());
    }

    // Handle actions
    match action.as_deref() {
        Some("delete") => {
            println!();
            println!("{}", "จะลบไฟล์ต่อไปนี้ (เก็บสำเนาแรกไว้):".yellow());
            for (_size, files) in &dupes {
                for f in files.iter().skip(1) {
                    println!("  {} {}", "🗑".red(), f.dimmed());
                }
            }
            let confirmed = Confirm::new()
                .with_prompt("ยืนยันการลบ?")
                .default(false)
                .interact()?;
            if !confirmed {
                println!("{}", "ยกเลิกแล้ว".dimmed());
                return Ok(());
            }
            let mut deleted = 0usize;
            for (_size, files) in &dupes {
                for f in files.iter().skip(1) {
                    let _ = std::fs::remove_file(f);
                    deleted += 1;
                }
            }
            println!("{} ลบ {} ไฟล์แล้ว", "✓".green(), deleted);
        }
        Some("move") => {
            let dup_dir = root.join(".duplicates");
            std::fs::create_dir_all(&dup_dir)?;
            let mut moved = 0usize;
            for (_size, files) in &dupes {
                for f in files.iter().skip(1) {
                    let src = std::path::Path::new(f);
                    let name = src.file_name().unwrap();
                    let dest = dup_dir.join(name);
                    let _ = std::fs::rename(src, &dest);
                    println!("  {} {} -> {}", "→".cyan(), f.dimmed(), dest.display());
                    moved += 1;
                }
            }
            println!("{} ย้าย {} ไฟล์ไปที่ {}", "✓".green(), moved, dup_dir.display());
        }
        Some("copy") => {
            let dup_dir = root.join(".duplicates");
            std::fs::create_dir_all(&dup_dir)?;
            let mut copied = 0usize;
            for (_size, files) in &dupes {
                for f in files.iter().skip(1) {
                    let src = std::path::Path::new(f);
                    let name = src.file_name().unwrap();
                    let dest = dup_dir.join(name);
                    let _ = std::fs::copy(src, &dest);
                    println!("  {} {} -> {}", "→".cyan(), f.dimmed(), dest.display());
                    copied += 1;
                }
            }
            println!(
                "{} คัดลอก {} ไฟล์ไปที่ {}",
                "✓".green(),
                copied,
                dup_dir.display()
            );
        }
        _ => {
            println!("{}", "Actions: --action delete|move|copy".dimmed());
        }
    }

    Ok(())
}

fn cmd_git(
    status: bool,
    add: bool,
    commit: Option<String>,
    push: bool,
    pull: bool,
    log: bool,
) -> AppResult<()> {
    let root = config::workspace_root();

    // Default: show status
    if !status && !add && commit.is_none() && !push && !pull && !log {
        let output = std::process::Command::new("git")
            .args(["status", "--short"])
            .current_dir(&root)
            .output()
            .map_err(|e| AppError::Message(format!("git ไม่พบ: {}", e)))?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
        return Ok(());
    }

    if status {
        let output = std::process::Command::new("git")
            .args(["status"])
            .current_dir(&root)
            .output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    if add {
        let output = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&root)
            .output()?;
        if output.status.success() {
            println!("{} {}", "✓".green(), "git add -A".dimmed());
        }
    }

    if let Some(msg) = commit {
        let output = std::process::Command::new("git")
            .args(["commit", "-m", &msg])
            .current_dir(&root)
            .output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    if push {
        let output = std::process::Command::new("git")
            .args(["push"])
            .current_dir(&root)
            .output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    if pull {
        let output = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(&root)
            .output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    if log {
        let output = std::process::Command::new("git")
            .args(["log", "--oneline", "-20"])
            .current_dir(&root)
            .output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    Ok(())
}

fn load_ignore_patterns() -> Vec<String> {
    let ignore_file = config::workspace_root().join(".file-cli/ignore");
    if ignore_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&ignore_file) {
            return content
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .map(|l| l.trim().to_string())
                .collect();
        }
    }
    SKIP_DIRS.iter().map(|s| s.to_string()).collect()
}

fn cmd_ignore(list: bool, add: Option<String>, remove: Option<String>) -> AppResult<()> {
    let ignore_file = config::workspace_root().join(".file-cli/ignore");

    if list {
        let patterns = load_ignore_patterns();
        println!("{}", header("Ignore Patterns"));
        for p in &patterns {
            println!("  {}", p);
        }
        println!();
        println!("{} patterns ({} built-in)", patterns.len(), SKIP_DIRS.len());
        return Ok(());
    }

    if let Some(pattern) = add {
        let mut patterns = load_ignore_patterns();
        if patterns.contains(&pattern) {
            println!("{} {} มีอยู่แล้ว", "⚠".yellow(), pattern);
            return Ok(());
        }
        patterns.push(pattern.clone());
        // Remove built-in from list before saving (they're always loaded)
        let custom: Vec<_> = patterns
            .into_iter()
            .filter(|p| !SKIP_DIRS.contains(&p.as_str()))
            .collect();
        let content: Vec<_> = custom.iter().map(|s| s.as_str()).collect();
        std::fs::write(&ignore_file, content.join("\n"))?;
        println!("{} {}", "✓".green(), format!("เพิ่ม ignore: {}", pattern));
        return Ok(());
    }

    if let Some(pattern) = remove {
        let patterns = load_ignore_patterns();
        let custom: Vec<_> = patterns
            .into_iter()
            .filter(|p| p != &pattern && !SKIP_DIRS.contains(&p.as_str()))
            .collect();
        let content: Vec<_> = custom.iter().map(|s| s.as_str()).collect();
        std::fs::write(&ignore_file, content.join("\n"))?;
        println!("{} {}", "✓".green(), format!("ลบ ignore: {}", pattern));
        return Ok(());
    }

    // Default: show help
    println!("{}", "Ignore — จัดการ pattern ที่ไม่ต้องการสแกน".cyan().bold());
    println!();
    println!(
        "  {} {}",
        "filz ignore --list".bold(),
        "แสดง patterns ทั้งหมด"
    );
    println!(
        "  {} {}",
        "filz ignore --add node_modules".bold(),
        "เพิ่ม pattern"
    );
    println!("  {} {}", "filz ignore --remove dist".bold(), "ลบ pattern");
    println!();
    println!(" Built-in: {}", SKIP_DIRS.join(", ").dimmed());
    Ok(())
}

fn cmd_set(name: String, type_: Option<String>, path: Option<PathBuf>) -> AppResult<()> {
    let mut reg = load_registry()?;
    let item = reg
        .items
        .get_mut(&name)
        .ok_or_else(|| AppError::Message(format!("ไม่พบ item: {}", name)))?;

    if let Some(t) = type_ {
        item.type_ = t.to_lowercase();
    }
    if let Some(p) = path {
        item.path = p.to_string_lossy().to_string();
    }

    let t = item.type_.clone();
    save_registry(&reg)?;

    println!("{}", format!("✓ แก้ {} -> [{}]", name, t).green().bold());
    Ok(())
}

fn cmd_check() -> AppResult<()> {
    let reg = load_registry()?;
    let mut issues = Vec::new();

    // Check for missing paths
    for (name, item) in &reg.items {
        let p = Path::new(&item.path);
        if !p.exists() {
            issues.push(format!(
                "{}: path ไม่พบ ({})",
                name.bold(),
                item.path.dimmed()
            ));
        }
    }

    // Check for duplicate paths
    let mut paths: Vec<(&str, &str)> = reg
        .items
        .values()
        .map(|it| (it.name.as_str(), it.path.as_str()))
        .collect();
    paths.sort_by(|a, b| a.1.cmp(b.1));
    for i in 1..paths.len() {
        if paths[i].1 == paths[i - 1].1 {
            issues.push(format!(
                "{}: path ซ้ำกับ {} ({})",
                paths[i].0.bold(),
                paths[i - 1].0.bold(),
                paths[i].1.dimmed()
            ));
        }
    }

    if issues.is_empty() {
        println!("{}", "✓ Registry ถูกต้อง ไม่มีปัญหา".green().bold());
    } else {
        println!("{}", header(&format!("พบ {} ปัญหา", issues.len())));
        for issue in &issues {
            println!("  {}", issue);
        }
    }

    println!("{}", format!("{} items ทั้งหมด", reg.items.len()).dimmed());
    Ok(())
}

fn cmd_list(all: bool, r#type: Option<String>) -> AppResult<()> {
    let reg = load_registry()?;

    if reg.items.is_empty() {
        println!("{}", "registry ว่าง — ใช้ 'filz new' สร้างรายการแรก".yellow());
        return Ok(());
    }

    let mut items: Vec<&Item> = reg.items.values().collect();

    if !all && r#type.is_none() {
        // No filters: show non-archived items only
        items.retain(|it| !it.category_path.contains("archive"));
    }

    if let Some(t) = r#type {
        let t = t.to_lowercase();
        items.retain(|i| i.type_ == t || i.category_path.contains(&t));
    }

    items.sort_by(|a, b| a.type_.cmp(&b.type_).then(a.name.cmp(&b.name)));

    if items.is_empty() {
        println!("{}", "ไม่พบรายการตามเงื่อนไข".yellow());
        println!("{}", "ลองใช้ 'filz list --all' เพื่อดูทั้งหมด".dimmed());
        return Ok(());
    }

    println!("{}", format!("=== {} items ===", items.len()).cyan().bold());

    for it in items {
        let size_mb = bytes_to_mb(dir_size_bytes(Path::new(&it.path)));
        println!(
            "{} [{}] ({:.2} MB) -> {}",
            it.name.bold(),
            color_for_type(&it.type_),
            size_mb,
            it.path.dimmed()
        );
    }

    if !all {
        println!();
        println!("{}", "ใช้ --all เพื่อดูรายการทั้งหมดรวม archive".dimmed());
    }

    Ok(())
}

// Directories to skip during traversal (show entry, don't recurse)
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    ".next",
    "dist",
    "build",
    ".turbo",
    ".vercel",
    "__pycache__",
    ".venv",
    ".mypy_cache",
    ".pytest_cache",
    "vendor",
    ".cache",
    ".idea",
    ".vscode",
];

fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn cmd_glob(pattern: String, path: Option<PathBuf>) -> AppResult<()> {
    let root =
        path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let cache_root = config::workspace_root();
    let cache_path = SearchCache::cache_path(&cache_root);
    let mut cache = SearchCache::load(&cache_path);

    if let Some(cached) = cache.cached_results(&pattern) {
        println!("{}", "ใช้ผลลัพธ์จาก cache".dimmed());
        for item in cached {
            println!("{}", item);
        }
        return Ok(());
    }

    println!(
        "{} ค้นหาไฟล์ที่ชื่อตรงกับ '{}' ใน {}",
        "🔍".cyan(),
        pattern,
        root.display()
    );
    let pat = pattern.to_lowercase();
    let mut count = 0usize;

    fn glob_match(name: &str, pat: &str) -> bool {
        // Simple glob:支持 * and ?
        let parts: Vec<&str> = pat.split('*').collect();
        if parts.len() == 1 {
            // No wildcard — check if name contains the pattern
            return name.to_lowercase().contains(pat);
        }
        // Has wildcards — check each part in order
        let mut pos = 0usize;
        for part in parts.iter() {
            if part.is_empty() {
                continue;
            }
            let part_lower = part.to_lowercase();
            match name.to_lowercase()[pos..].find(&part_lower) {
                Some(found) => {
                    pos += found + part.len();
                }
                None => return false,
            }
        }
        true
    }

    let count = with_spinner("searching files", || {
        fn walk(dir: &Path, pat: &str, count: &mut usize) {
            if let Ok(entries) = fs::read_dir(dir) {
                let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                entries.sort_by_key(|e| e.file_name());
                for entry in entries {
                    let name = entry.file_name();
                    let display = name.to_string_lossy();
                    let path = entry.path();

                    if should_skip_dir(&display) {
                        println!("{}/  (skipped)", colorize_dir(&display));
                        continue;
                    }
                    if is_hidden(&display) && !pat.starts_with('.') {
                        continue;
                    }

                    if path.is_dir() {
                        walk(&path, pat, count);
                    } else if glob_match(&display, pat) {
                        println!("{}", colorize_file(&display));
                        *count += 1;
                    }
                }
            }
        }

        walk(&root, &pat, &mut count);
        let mut results = Vec::new();
        if count > 0 {
            let mut collected = Vec::new();
            fn collect_matches(dir: &Path, pat: &str, out: &mut Vec<String>, limit: usize) {
                if out.len() >= limit {
                    return;
                }
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        let display = path.display().to_string();
                        if path.is_dir() {
                            collect_matches(&path, pat, out, limit);
                        } else if glob_match(&display, pat) {
                            out.push(display);
                        }
                    }
                }
            }
            collect_matches(&root, &pat, &mut collected, 50);
            results = collected;
            if !results.is_empty() {
                cache.remember(&pattern, &results);
                let _ = cache.save(&cache_path);
            }
        }
        Ok((count, results))
    })?;
    if count.0 == 0 {
        println!(
            "{}",
            format!("ไม่เจอ '{}' ใน {}", pattern, root.display()).yellow()
        );
    } else {
        for item in count.1 {
            println!("{}", item);
        }
    }
    Ok(())
}

fn cmd_grep(pattern: String, path: Option<PathBuf>) -> AppResult<()> {
    let root = path
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    println!("{} ค้นหาข้อความ '{}' ในเนื้อหาไฟล์", "🔎".cyan(), pattern);
    let rg_path = config::vendor_bin("rg");

    if let Some(rg) = rg_path {
        let output = std::process::Command::new(&rg)
            .args([
                "--color=always",
                "--line-number",
                "--hidden",
                "-g",
                "!node_modules",
                "-g",
                "!.git",
                "-g",
                "!target",
                &pattern,
            ])
            .arg(&root)
            .output()
            .map_err(|e| AppError::Message(format!("rg ไม่พบ: {}", e)))?;

        if output.stdout.is_empty() {
            println!("{}", format!("ไม่เจอ '{}'", pattern).yellow());
        } else {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        return Ok(());
    }

    // Fallback: pure Rust grep (no rg available)
    let root = path.unwrap_or_else(|| PathBuf::from("."));
    let pat = pattern.to_lowercase();
    let count = with_spinner("searching file contents", || {
        let mut count = 0usize;
        fn walk_grep(dir: &Path, pat: &str, count: &mut usize) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name();
                    let display = name.to_string_lossy();
                    let path = entry.path();

                    if should_skip_dir(&display) {
                        continue;
                    }
                    if path.is_dir() {
                        walk_grep(&path, pat, count);
                    } else if let Ok(content) = fs::read_to_string(&path) {
                        for (i, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(pat) {
                                println!("{}:{}:{}", path.display(), i + 1, line);
                                *count += 1;
                            }
                        }
                    }
                }
            }
        }

        walk_grep(&root, &pat, &mut count);
        Ok(count)
    })?;
    if count == 0 {
        println!("{}", format!("ไม่เจอ '{}'", pattern).yellow());
    }
    Ok(())
}

fn cmd_templates() -> AppResult<()> {
    let templates = load_templates();

    if templates.is_empty() {
        println!("{}", "ไม่พบ template ใน templates/".yellow());
        println!(
            "{}",
            "สร้างไฟล์ YAML ใน templates/ เช่น templates/website.yaml".dimmed()
        );
        return Ok(());
    }

    render_banner(&format!("Templates ({})", templates.len()));

    for t in &templates {
        println!(
            "  {} {} - {} [{}:{}]",
            t.name.bold(),
            t.description.dimmed(),
            t.r#type.dimmed(),
            t.stack.dimmed(),
            format!("{} files", t.files.len()).dimmed()
        );
    }

    println!();
    println!("{}", "ใช้ wizard แบบ step-by-step:".bold());
    println!(
        "  {} — สร้าง project ใหม่ด้วย template",
        "filz new --template website".green()
    );
    println!(
        "  {} — สร้าง project แบบ interactive (มี wizard)",
        "filz new".green()
    );
    println!();
    println!("{}", "ใช้ --yes เพื่อข้าม wizard และสร้างทันที:".dimmed());
    println!(
        "  {} — สร้างโดยไม่ต้องยืนยัน",
        "filz new --name myapp --type work --template website --yes".dimmed()
    );

    Ok(())
}

fn cmd_find(query: String, verbose: bool) -> AppResult<()> {
    let reg = load_registry()?;
    let q = query.to_lowercase();

    let mut hits: Vec<&Item> = reg
        .items
        .values()
        .filter(|it| {
            it.name.to_lowercase().contains(&q)
                || it.type_.to_lowercase().contains(&q)
                || it.path.to_lowercase().contains(&q)
                || it.category_path.to_lowercase().contains(&q)
        })
        .collect();

    hits.sort_by(|a, b| a.name.cmp(&b.name));

    if hits.is_empty() {
        println!("{}", format!("ไม่เจอ '{}'", query).yellow());
        println!("{}", "ลองใช้ 'filz list' เพื่อดูรายการทั้งหมด".dimmed());
        return Ok(());
    }

    render_banner(&format!("Results for '{}'", query));
    println!(
        "{}",
        format!("พบ {} รายการสำหรับ '{}':", hits.len(), query)
            .green()
            .bold()
    );
    for it in hits {
        let size_mb = bytes_to_mb(dir_size_bytes(Path::new(&it.path)));
        if verbose {
            println!(
                "  {} [{}] {} {:.2} MB -> {}",
                it.name.cyan().bold(),
                it.type_,
                it.stack.dimmed(),
                size_mb,
                it.path.dimmed()
            );
        } else {
            println!(
                "  {} [{}] {:.2} MB -> {}",
                it.name.cyan().bold(),
                it.type_,
                size_mb,
                it.path.dimmed()
            );
        }
    }

    Ok(())
}

fn cmd_archive(
    theme: &ColorfulTheme,
    name: Option<String>,
    path: Option<PathBuf>,
    format: ArchiveFormat,
    output: Option<PathBuf>,
    yes: bool,
) -> AppResult<()> {
    let reg = load_registry()?;
    let target_path = if let Some(p) = path {
        p
    } else if let Some(n) = name {
        let n = sanitize_name(&n)?;
        if let Some(item) = reg.items.get(&n) {
            PathBuf::from(&item.path)
        } else {
            return Err(AppError::Message(format!("ไม่พบ item ชื่อ {}", n)));
        }
    } else {
        let names: Vec<String> = reg.items.keys().cloned().collect();
        if names.is_empty() {
            return Err(AppError::Message("registry ว่าง".into()));
        }
        let idx = Select::with_theme(theme)
            .with_prompt("เลือก item ที่จะ archive")
            .items(&names)
            .default(0)
            .interact()?;
        PathBuf::from(&reg.items[&names[idx]].path)
    };

    if !target_path.exists() {
        return Err(AppError::Message(format!(
            "path ไม่พบ: {}",
            target_path.display()
        )));
    }

    let folder_name = target_path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or_else(|| AppError::Message("ชื่อโฟลเดอร์ไม่ถูกต้อง".into()))?
        .to_string();

    // Determine output path
    let archive_root = workspace_root().join("04-archive");
    ensure_dir(&archive_root)?;

    let ext = match format {
        ArchiveFormat::Zip => "zip",
        ArchiveFormat::Tar => "tar",
        ArchiveFormat::Targz => "tar.gz",
        ArchiveFormat::Sevenz => "7z",
    };

    let dest = if let Some(o) = output {
        if o.is_dir() {
            o.join(format!("{}.{}", folder_name, ext))
        } else {
            o
        }
    } else {
        archive_root.join(format!("{}.{}", folder_name, ext))
    };

    if dest.exists() {
        return Err(AppError::Message(format!("ไฟล์มีอยู่แล้ว: {}", dest.display())));
    }

    // Preview (always show what will happen)
    let source_size = dir_size_bytes(&target_path);
    println!();
    println!("{}", "╔══════════════════════════════════════╗".cyan());
    println!(
        "{}",
        "║         Preview: Archive               ║".cyan().bold()
    );
    println!("{}", "╠══════════════════════════════════════╣".cyan());
    println!("{}  {}", "  ต้นทาง:".bold(), target_path.display());
    println!("{}  {}", "  ปลายทาง:".bold(), dest.display());
    println!("{}  {}", "  รูปแบบ:", format);
    println!("{}  {:.2} MB", "  ขนาด:".bold(), bytes_to_mb(source_size));
    println!("{}", "╚══════════════════════════════════════╝".cyan());
    println!();

    if !yes {
        let confirmed = Confirm::with_theme(theme)
            .with_prompt("สร้าง archive นี้?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "ยกเลิกแล้ว".dimmed());
            return Ok(());
        }
    }

    // Compress: try system tool first, fall back to Rust crate
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    let dir_name = target_path.file_name().unwrap();

    let _used_rust = with_spinner("compressing archive", || {
        Ok(match format {
            ArchiveFormat::Zip => {
                let status = std::process::Command::new("zip")
                    .args(["-r", &dest.to_string_lossy(), &dir_name.to_string_lossy()])
                    .current_dir(parent)
                    .status();
                match status {
                    Ok(s) if s.success() => false,
                    _ => {
                        println!("{}", "  ใช้ built-in zip...".dimmed());
                        let file = std::fs::File::create(&dest)
                            .map_err(|e| AppError::Message(format!("สร้างไฟล์ไม่ได้: {}", e)))?;
                        let mut zip = zip::ZipWriter::new(file);
                        let options = zip::write::SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Deflated);
                        fn add_to_zip(
                            dir: &Path,
                            zip: &mut zip::ZipWriter<std::fs::File>,
                            base: &Path,
                            opts: &zip::write::SimpleFileOptions,
                        ) -> Result<(), AppError> {
                            for entry in std::fs::read_dir(dir)
                                .map_err(|e| AppError::Message(e.to_string()))?
                            {
                                let entry = entry.map_err(|e| AppError::Message(e.to_string()))?;
                                let path = entry.path();
                                let rel = path.strip_prefix(base).unwrap_or(&path);
                                if path.is_dir() {
                                    zip.add_directory(rel.to_string_lossy().as_ref(), opts.clone())
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                    add_to_zip(&path, zip, base, opts)?;
                                } else {
                                    zip.start_file(rel.to_string_lossy().as_ref(), opts.clone())
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                    let mut f = std::fs::File::open(&path)
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                    std::io::copy(&mut f, zip)
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                }
                            }
                            Ok(())
                        }
                        add_to_zip(&target_path, &mut zip, &target_path, &options)?;
                        zip.finish().map_err(|e| AppError::Message(e.to_string()))?;
                        true
                    }
                }
            }
            ArchiveFormat::Tar => {
                let status = std::process::Command::new("tar")
                    .args(["cf", &dest.to_string_lossy(), &dir_name.to_string_lossy()])
                    .current_dir(parent)
                    .status();
                match status {
                    Ok(s) if s.success() => false,
                    _ => {
                        println!("{}", "  ใช้ built-in tar...".dimmed());
                        let f = std::fs::File::create(&dest)
                            .map_err(|e| AppError::Message(format!("สร้างไฟล์ไม่ได้: {}", e)))?;
                        let mut ar = tar::Builder::new(f);
                        ar.append_dir_all(dir_name, &target_path)
                            .map_err(|e| AppError::Message(e.to_string()))?;
                        ar.finish().map_err(|e| AppError::Message(e.to_string()))?;
                        true
                    }
                }
            }
            ArchiveFormat::Targz => {
                let status = std::process::Command::new("tar")
                    .args(["czf", &dest.to_string_lossy(), &dir_name.to_string_lossy()])
                    .current_dir(parent)
                    .status();
                match status {
                    Ok(s) if s.success() => false,
                    _ => {
                        println!("{}", "  ใช้ built-in tar+gzip...".dimmed());
                        let f = std::fs::File::create(&dest)
                            .map_err(|e| AppError::Message(format!("สร้างไฟล์ไม่ได้: {}", e)))?;
                        let enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
                        let mut ar = tar::Builder::new(enc);
                        ar.append_dir_all(dir_name, &target_path)
                            .map_err(|e| AppError::Message(e.to_string()))?;
                        ar.finish().map_err(|e| AppError::Message(e.to_string()))?;
                        true
                    }
                }
            }
            ArchiveFormat::Sevenz => {
                let status = std::process::Command::new("7z")
                    .args(["a", &dest.to_string_lossy(), &dir_name.to_string_lossy()])
                    .current_dir(parent)
                    .status();
                match status {
                    Ok(s) if s.success() => false,
                    _ => {
                        println!("{}", "  7z ไม่มี — ใช้ zip แทน (รองรับทุก OS)".yellow());
                        let file = std::fs::File::create(&dest)
                            .map_err(|e| AppError::Message(format!("สร้างไฟล์ไม่ได้: {}", e)))?;
                        let mut zip = zip::ZipWriter::new(file);
                        let options = zip::write::SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Deflated);
                        fn add_to_zip(
                            dir: &Path,
                            zip: &mut zip::ZipWriter<std::fs::File>,
                            base: &Path,
                            opts: &zip::write::SimpleFileOptions,
                        ) -> Result<(), AppError> {
                            for entry in std::fs::read_dir(dir)
                                .map_err(|e| AppError::Message(e.to_string()))?
                            {
                                let entry = entry.map_err(|e| AppError::Message(e.to_string()))?;
                                let path = entry.path();
                                let rel = path.strip_prefix(base).unwrap_or(&path);
                                if path.is_dir() {
                                    zip.add_directory(rel.to_string_lossy().as_ref(), opts.clone())
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                    add_to_zip(&path, zip, base, opts)?;
                                } else {
                                    zip.start_file(rel.to_string_lossy().as_ref(), opts.clone())
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                    let mut f = std::fs::File::open(&path)
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                    std::io::copy(&mut f, zip)
                                        .map_err(|e| AppError::Message(e.to_string()))?;
                                }
                            }
                            Ok(())
                        }
                        add_to_zip(&target_path, &mut zip, &target_path, &options)?;
                        zip.finish().map_err(|e| AppError::Message(e.to_string()))?;
                        true
                    }
                }
            }
        })
    })?;

    // Show result
    let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    let size_str = if size >= 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KB", size as f64 / 1024.0)
    };

    println!(
        "{}",
        format!("✓ archive [{}] {} -> {}", format, size_str, dest.display())
            .green()
            .bold()
    );
    Ok(())
}

fn cmd_clean(theme: &ColorfulTheme, yes: bool) -> AppResult<()> {
    println!("{} ล้างไฟล์ขยะ (preview ก่อนเสมอ)", "🧹".cyan().bold());
    let reg = load_registry()?;
    let targets: Vec<Item> = reg.items.values().cloned().collect();

    if targets.is_empty() {
        println!("{}", "ไม่มี target ให้ล้าง".yellow());
        return Ok(());
    }

    let categories = vec![
        ("Node: node_modules", vec!["node_modules"], vec![]),
        (
            "Build: .next / dist / build / .turbo / .vercel",
            vec![".next", "dist", "build", ".turbo", ".vercel"],
            vec![],
        ),
        ("Rust: target/", vec!["target"], vec![]),
        (
            "Python: __pycache__ / .venv / .pytest_cache / .mypy_cache",
            vec!["__pycache__", ".venv", ".pytest_cache", ".mypy_cache"],
            vec![],
        ),
        ("Go: vendor / bin", vec!["vendor", "bin"], vec![]),
        ("Tauri: src-tauri/target", vec!["src-tauri/target"], vec![]),
        (
            "Logs & Temp",
            vec!["logs", "tmp", ".tmp"],
            vec![
                "npm-debug.log",
                "yarn-error.log",
                "Thumbs.db",
                ".DS_Store",
                "desktop.ini",
            ],
        ),
    ];

    let labels: Vec<String> = categories.iter().map(|(l, _, _)| l.to_string()).collect();

    let selected = if yes {
        (0..categories.len()).collect::<Vec<_>>()
    } else {
        MultiSelect::with_theme(theme)
            .with_prompt("เลือกหมวดที่จะล้าง")
            .items(&labels)
            .defaults(&vec![true, true, true, true, false, false, true])
            .interact()?
    };

    if selected.is_empty() {
        println!("{}", "ไม่ได้เลือกอะไร - ยกเลิก".yellow());
        return Ok(());
    }

    let mut to_delete: HashSet<PathBuf> = HashSet::new();

    let selected = if yes {
        (0..categories.len()).collect::<Vec<_>>()
    } else {
        MultiSelect::with_theme(theme)
            .with_prompt("เลือกหมวดที่จะล้าง")
            .items(&labels)
            .defaults(&vec![true, true, true, true, false, false, true])
            .interact()?
    };

    if selected.is_empty() {
        println!("{}", "ไม่ได้เลือกอะไร - ยกเลิก".yellow());
        return Ok(());
    }

    let _ = with_spinner("scanning for junk files", || {
        for idx in &selected {
            let (_, dirs, files) = &categories[*idx];

            for item in &targets {
                let base = PathBuf::from(&item.path);

                for d in dirs {
                    let p = base.join(d);
                    if p.exists() {
                        to_delete.insert(p);
                    }

                    if let Ok(entries) = fs::read_dir(&base) {
                        for e in entries.filter_map(|e| e.ok()) {
                            let nested = e.path().join(d);
                            if nested.exists() {
                                to_delete.insert(nested);
                            }
                        }
                    }
                }

                for f in files {
                    let p = base.join(f);
                    if p.exists() {
                        to_delete.insert(p);
                    }
                }
            }
        }
        Ok(())
    })?;

    let total_freed_mb: f64 = to_delete
        .iter()
        .map(|p| bytes_to_mb(dir_size_bytes(p)))
        .sum();

    println!();
    println!("{}", "Preview:".yellow().bold());
    println!(
        "{}",
        format!(
            "จะลบ {} รายการ คืนพื้นที่ ~{:.2} MB",
            to_delete.len(),
            total_freed_mb
        )
        .yellow()
    );

    for p in to_delete.iter().take(10) {
        println!("  - {}", p.display());
    }

    if to_delete.len() > 10 {
        println!("  ... และอีก {} รายการ", to_delete.len() - 10);
    }

    let confirmed = Confirm::with_theme(theme)
        .with_prompt(format!("ยืนยันลบทั้งหมด ~{:.2} MB ?", total_freed_mb))
        .default(false)
        .interact()?;

    if !confirmed {
        println!("{}", "ยกเลิกแล้ว".dimmed());
        return Ok(());
    }

    let mut freed_actual = 0.0;
    for p in to_delete {
        if p.is_dir() {
            freed_actual += bytes_to_mb(dir_size_bytes(&p));
            fs::remove_dir_all(&p)?;
        } else {
            freed_actual += bytes_to_mb(fs::metadata(&p)?.len());
            fs::remove_file(&p)?;
        }
    }

    println!(
        "{}",
        format!("✓ ล้างเสร็จ คืนมา {:.2} MB", freed_actual)
            .green()
            .bold()
    );
    Ok(())
}

// --- Tool detection ---

fn tool_version(cmd: &str, flag: &str) -> (bool, String, String) {
    // Returns (installed, version_string, path)
    // Check vendor first, then system PATH
    let bin = config::vendor_bin(cmd);
    let result = if let Some(ref p) = bin {
        std::process::Command::new(p).arg(flag).output()
    } else {
        std::process::Command::new(cmd).arg(flag).output()
    };

    match result {
        Ok(o) if o.status.success() => {
            let out = String::from_utf8_lossy(&o.stdout);
            let ver = out.lines().next().unwrap_or("").trim().to_string();
            let path = bin
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            (true, ver, path)
        }
        _ => (false, String::new(), String::new()),
    }
}

struct DetectedTool {
    name: &'static str,
    installed: bool,
    version: String,
    #[allow(dead_code)] // used for proto serialization later
    path: String,
}

fn detect_tools() -> Vec<DetectedTool> {
    // (name, version_flag)
    let checks: &[(&str, &[&str])] = &[
        ("rustc", &["--version"]),
        ("cargo", &["--version"]),
        ("python3", &["--version"]),
        ("python", &["--version"]),
        ("git", &["--version"]),
        ("gh", &["--version"]),
        ("node", &["--version"]),
        ("bun", &["--version"]),
        ("go", &["version"]),
        ("npm", &["--version"]),
        ("pnpm", &["--version"]),
        ("yarn", &["--version"]),
        ("protoc", &["--version"]),
    ];

    let mut seen = HashSet::new();
    let mut tools = Vec::new();

    for &(name, flags) in checks {
        if seen.contains(name) {
            continue;
        }
        let flag = flags[0];
        let (installed, version, path) = tool_version(name, flag);
        if installed {
            seen.insert(name);
        }
        tools.push(DetectedTool {
            name,
            installed,
            version,
            path,
        });
    }

    tools
}

fn detect_shell() -> String {
    std::env::var("SHELL")
        .or_else(|_| std::env::var("ComSpec"))
        .unwrap_or_else(|_| "unknown".into())
        .split('/')
        .last()
        .unwrap_or("unknown")
        .to_string()
}

fn detect_os() -> String {
    std::env::consts::OS.to_string()
}

fn detect_arch() -> String {
    std::env::consts::ARCH.to_string()
}

fn cmd_tools() -> AppResult<()> {
    let tools = detect_tools();
    let shell = detect_shell();
    let os = detect_os();
    let arch = detect_arch();

    println!("{}", header("Toolchain Detection"));
    println!(
        "{} {} | {} | {}",
        "OS:".dimmed(),
        os.bold(),
        arch.bold(),
        format!("shell={}", shell).bold()
    );
    println!();

    let installed_count = tools.iter().filter(|t| t.installed).count();

    for t in &tools {
        if t.installed {
            println!(
                "  {} {} {}",
                "✓".green(),
                format!("{:>8}", t.name).bold(),
                t.version.dimmed()
            );
        }
    }

    let missing: Vec<_> = tools.iter().filter(|t| !t.installed).collect();
    if !missing.is_empty() {
        println!();
        println!("{}", "Not installed:".yellow());
        for t in &missing {
            println!("  {} {}", "✗".red(), format!("{:>8}", t.name).dimmed());
        }
    }

    println!();
    println!(
        "Summary: {}/{} tools installed",
        installed_count.to_string().green().bold(),
        tools.len()
    );

    Ok(())
}

fn cmd_doctor() -> AppResult<()> {
    let reg = load_registry()?;
    println!("{}", header("E: Drive Doctor"));

    if reg.items.is_empty() {
        println!("{}", "Registry ว่าง".yellow());
    } else {
        let mut total = 0u64;
        let mut bloated: Vec<(&Item, u64, &'static str)> = Vec::new();
        let mut healthy = 0usize;

        for it in reg.items.values() {
            let size = dir_size_bytes(Path::new(&it.path));
            total += size;

            if size > 1024 * 1024 * 1000 {
                bloated.push((it, size, "CRITICAL"));
            } else if size > 1024 * 1024 * 500 {
                bloated.push((it, size, "WARN"));
            } else {
                healthy += 1;
            }
        }

        bloated.sort_by(|a, b| b.1.cmp(&a.1));

        for (it, size, level) in bloated {
            let tag = if level == "CRITICAL" {
                "CRIT".red().bold()
            } else {
                "WARN".yellow().bold()
            };

            println!(
                "{} {} [{}:{}] บวม {:.2} MB -> {}",
                tag,
                it.name.bold(),
                it.type_,
                it.stack,
                bytes_to_mb(size),
                it.path.dimmed()
            );
        }

        println!();
        println!(
            "สรุป: ทั้งหมด {:.2} MB | Healthy {} items | ต้องดู {} items",
            bytes_to_mb(total),
            healthy,
            reg.items.len().saturating_sub(healthy)
        );
    }

    println!();
    println!("{}", "System Paths:".dimmed());
    let sys_paths = vec![
        (
            "E:/cache/pnpm-store",
            workspace_root().join("cache").join("pnpm-store"),
        ),
        (
            "E:/cache/cargo",
            workspace_root().join("cache").join("cargo"),
        ),
        ("E:/cache/pip", workspace_root().join("cache").join("pip")),
        ("E:/runtimes", workspace_root().join("runtimes")),
        ("E:/tools", workspace_root().join("tools")),
        ("E:/01-projects", workspace_root().join("01-projects")),
        ("E:/02-assets", workspace_root().join("02-assets")),
        ("E:/03-docs", workspace_root().join("03-docs")),
        ("E:/04-archive", workspace_root().join("04-archive")),
    ];

    for (label, p) in sys_paths {
        let sz = bytes_to_mb(dir_size_bytes(&p));
        let status = if sz > 5000.0 {
            "บวมมาก".red()
        } else if sz > 1000.0 {
            "เริ่มเยอะ".yellow()
        } else {
            "OK".green()
        };

        println!(
            "{} {} - {:.2} MB",
            format!("{:20}", label).dimmed(),
            status,
            sz
        );
    }

    Ok(())
}

fn cmd_system_status() -> AppResult<()> {
    let reg = load_registry()?;
    println!("{}", header("E: Drive Status"));
    println!("Registry: {} items", reg.items.len());
    println!(
        "Workspace: {} ({})",
        workspace_root().display(),
        if workspace_root().exists() {
            "OK".green()
        } else {
            "not found".yellow()
        }
    );
    println!(
        "Runtimes: {:.2} MB",
        bytes_to_mb(dir_size_bytes(&workspace_root().join("runtimes")))
    );
    println!(
        "Cache: {:.2} MB",
        bytes_to_mb(dir_size_bytes(&workspace_root().join("cache")))
    );
    println!(
        "Projects: {:.2} MB",
        bytes_to_mb(dir_size_bytes(&workspace_root().join("01-projects")))
    );
    println!(
        "Assets: {:.2} MB",
        bytes_to_mb(dir_size_bytes(&workspace_root().join("02-assets")))
    );
    println!(
        "Docs: {:.2} MB",
        bytes_to_mb(dir_size_bytes(&workspace_root().join("03-docs")))
    );
    println!(
        "Archive: {:.2} MB",
        bytes_to_mb(dir_size_bytes(&workspace_root().join("04-archive")))
    );
    Ok(())
}

fn cmd_system_clean(yes: bool) -> AppResult<()> {
    let caches = vec![
        workspace_root().join("cache").join("pnpm-store"),
        workspace_root().join("cache").join("cargo"),
        workspace_root().join("cache").join("pip"),
        workspace_root().join("cache").join("tmp"),
    ];

    let total: f64 = caches.iter().map(|p| bytes_to_mb(dir_size_bytes(p))).sum();

    println!("{}", format!("จะล้าง cache รวม ~{:.2} MB", total).yellow());

    if !yes {
        let confirmed = Confirm::new()
            .with_prompt("ยืนยันลบ cache ?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "ยกเลิกแล้ว".dimmed());
            return Ok(());
        }
    }

    for c in caches {
        if c.exists() {
            println!("ลบ {}", c.display());
            fs::remove_dir_all(&c)?;
        }
    }

    println!("{}", "✓ system clean เสร็จแล้ว".green().bold());
    Ok(())
}

fn print_completion(shell: Shell, output: Option<PathBuf>) -> AppResult<()> {
    let mut cmd = Cli::command();
    let mut buffer = Vec::new();

    match shell {
        Shell::Bash => clap_complete::generate(Bash, &mut cmd, APP_NAME, &mut buffer),
        Shell::Fish => clap_complete::generate(Fish, &mut cmd, APP_NAME, &mut buffer),
        Shell::Zsh => clap_complete::generate(Zsh, &mut cmd, APP_NAME, &mut buffer),
        Shell::PowerShell => clap_complete::generate(PowerShell, &mut cmd, APP_NAME, &mut buffer),
    }

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &buffer)?;
        println!("{} -> {}", "✓".green(), path.display());
    } else {
        std::io::stdout().write_all(&buffer).map_err(AppError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_empty() {
        assert!(sanitize_name("").is_err());
        assert!(sanitize_name("   ").is_err());
    }

    #[test]
    fn sanitize_rejects_bad_chars() {
        for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
            assert!(sanitize_name(&format!("bad{}name", c)).is_err());
        }
    }

    #[test]
    fn sanitize_trims() {
        assert_eq!(sanitize_name("  ok  ").unwrap(), "ok");
    }

    #[test]
    fn resolve_path_work_types() {
        let (path, cat) = resolve_path("work", "rust", "my-proj");
        assert!(path.ends_with("my-proj"));
        assert_eq!(cat, "01-projects/01-work");
    }

    #[test]
    fn resolve_path_asset_sub() {
        let (path, cat) = resolve_path("asset", "figma", "icons");
        assert!(path.to_string_lossy().contains("06-design"));
        assert_eq!(cat, "02-assets/06-design");
    }

    #[test]
    fn resolve_path_doc_sub() {
        let (path, _) = resolve_path("doc", "contract", "nda");
        assert!(path.to_string_lossy().contains("01-contracts"));
    }

    #[test]
    fn resolve_path_fallback() {
        let (path, cat) = resolve_path("unknown", "x", "p");
        assert!(path.to_string_lossy().contains("01-work"));
        assert_eq!(cat, "01-projects/01-work");
    }

    #[test]
    fn stacks_for_known_type() {
        let s = stacks_for_type("work");
        assert!(s.contains(&"rust"));
        assert!(s.contains(&"node"));
    }

    #[test]
    fn stacks_for_unknown_type() {
        let s = stacks_for_type("nope");
        assert!(s.contains(&"rust"));
    }

    #[test]
    fn bytes_to_mb_conversion() {
        assert!((bytes_to_mb(1024 * 1024) - 1.0).abs() < 0.001);
        assert_eq!(bytes_to_mb(0), 0.0);
    }

    fn build_registry(n: usize) -> Registry {
        let mut items = HashMap::new();
        for i in 0..n {
            let name = format!("item-{:07}", i);
            items.insert(
                name.clone(),
                Item {
                    name,
                    type_: "work".into(),
                    stack: "rust".into(),
                    category_path: "01-projects/01-work".into(),
                    path: format!("/data/projects/{:07}", i),
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
            );
        }
        Registry { items }
    }

    fn bench_scale(
        n: usize,
    ) -> (
        std::time::Duration,
        std::time::Duration,
        std::time::Duration,
        usize,
    ) {
        use std::time::Instant;

        let reg = build_registry(n);
        let tmp = std::env::temp_dir().join(format!("bench_{}.pb", n));

        // Encode + write
        let pb = reg.to_proto();
        let mut buf = Vec::new();
        pb.encode(&mut buf).unwrap();
        std::fs::write(&tmp, &buf).unwrap();
        let file_size = buf.len();

        // Read
        let t0 = Instant::now();
        let bytes = std::fs::read(&tmp).unwrap();
        let read_time = t0.elapsed();

        // Decode
        let t1 = Instant::now();
        let pb_reg = pb::Registry::decode(&bytes[..]).unwrap();
        let decoded = Registry::from_proto(pb_reg);
        let decode_time = t1.elapsed();

        // Search (1000 lookups)
        let t2 = Instant::now();
        for i in 0..1000 {
            let key = format!("item-{:07}", i % n);
            let _ = decoded.items.get(&key);
        }
        let search_time = t2.elapsed();

        let _ = std::fs::remove_file(&tmp);
        (read_time, decode_time, search_time, file_size)
    }

    #[test]
    fn bench_registry_1k() {
        let (read, decode, search, size) = bench_scale(1000);
        println!("=== 1K items ({} KB) ===", size / 1024);
        println!("  Read: {:>8.2?}", read);
        println!("  Decode: {:>8.2?}", decode);
        println!("  Search x1000: {:>8.2?}", search);
        let cycle = read + decode + search / 1000;
        println!("  Per-search cycle: {:>8.2?}", cycle);
        assert!(cycle < std::time::Duration::from_millis(250));
    }

    #[test]
    fn bench_registry_10k() {
        let (read, decode, search, size) = bench_scale(10_000);
        println!("=== 10K items ({} KB) ===", size / 1024);
        println!("  Read: {:>8.2?}", read);
        println!("  Decode: {:>8.2?}", decode);
        println!("  Search x1000: {:>8.2?}", search);
        let cycle = read + decode + search / 1000;
        println!("  Per-search cycle: {:>8.2?}", cycle);
        assert!(cycle < std::time::Duration::from_millis(500));
    }

    #[test]
    fn bench_registry_100k() {
        let (read, decode, search, size) = bench_scale(100_000);
        println!("=== 100K items ({} KB) ===", size / 1024);
        println!("  Read: {:>8.2?}", read);
        println!("  Decode: {:>8.2?}", decode);
        println!("  Search x1000: {:>8.2?}", search);
        let cycle = read + decode + search / 1000;
        println!("  Per-search cycle: {:>8.2?}", cycle);
        assert!(cycle < std::time::Duration::from_millis(3000));
    }

    #[test]
    fn bench_registry_1m() {
        let (read, decode, search, size) = bench_scale(1_000_000);
        println!("=== 1M items ({} MB) ===", size / (1024 * 1024));
        println!("  Read: {:>8.2?}", read);
        println!("  Decode: {:>8.2?}", decode);
        println!("  Search x1000: {:>8.2?}", search);
        let cycle = read + decode + search / 1000;
        println!("  Per-search cycle: {:>8.2?}", cycle);
        assert!(cycle < std::time::Duration::from_secs(20));
    }
}
