mod fuzzy;
mod input;
mod selector;
mod tui;

use chrono::Local;
use input::{parse_test_keys, KeyInput};
use rand::Rng;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tui::{disable_colors, Terminal};

const VERSION: &str = "2.0.0";
#[allow(dead_code)]
const SCRIPT_WARNING: &str =
    "# if you can read this, you didn't launch try from an alias. run try --help.";

#[derive(Debug, Clone)]
struct CliOptions {
    tries_path: PathBuf,
    no_expand_tokens: bool,
    and_exit: bool,
    and_keys: Vec<KeyInput>,
    and_type: Option<String>,
    and_confirm: bool,
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();

    let no_colors = extract_flag(&mut args, "--no-colors");
    let no_expand_tokens = extract_flag(&mut args, "--no-expand-tokens");
    if no_colors || no_expand_tokens || no_color_env_set() {
        disable_colors();
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprint!("{}", global_help_text());
        std::process::exit(0);
    }

    if args.iter().any(|a| a == "--version" || a == "-v") {
        eprintln!("try {}", VERSION);
        std::process::exit(0);
    }

    let tries_path = extract_option_with_value(&mut args, "--path")
        .map(|v| expand_tilde_path(&v))
        .unwrap_or_else(|| expand_tilde_path("~/.try"));

    let and_type = extract_option_with_value(&mut args, "--and-type");
    let and_exit = extract_flag(&mut args, "--and-exit");
    let and_keys_raw = extract_option_with_value(&mut args, "--and-keys");
    let and_confirm = extract_flag(&mut args, "--and-confirm")
        || extract_option_with_value(&mut args, "--and-confirm").is_some();

    let options = CliOptions {
        tries_path,
        no_expand_tokens,
        and_exit,
        and_keys: and_keys_raw
            .as_deref()
            .map(parse_test_keys)
            .unwrap_or_default(),
        and_type,
        and_confirm,
    };

    let command = args.first().cloned();
    let rest = if args.is_empty() {
        Vec::new()
    } else {
        args[1..].to_vec()
    };

    dispatch(command, rest, options);
}

fn dispatch(command: Option<String>, rest: Vec<String>, options: CliOptions) {
    match command.as_deref() {
        None => {
            if stdin_is_tty() {
                eprint!("{}", global_help_text());
                std::process::exit(0);
            }
            std::process::exit(2);
        }
        Some("init") => cmd_init(rest, options),
        Some("install") => cmd_install(rest, options),
        Some("exec") => cmd_exec(rest, options),
        Some("clone") => cmd_clone(rest, options),
        Some("worktree") => cmd_worktree(rest, options),
        Some(".") => cmd_dot(rest, options),
        Some(path) if path.starts_with("./") => cmd_dot_with_path(path.to_string(), rest, options),
        Some(query) => cmd_query(query.to_string(), rest, options),
    }
}

fn extract_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let mut seen = false;
    args.retain(|arg| {
        if arg == flag {
            seen = true;
            false
        } else {
            true
        }
    });
    seen
}

fn extract_option_with_value(args: &mut Vec<String>, opt_name: &str) -> Option<String> {
    let mut last_value: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == opt_name {
            args.remove(i);
            let value = if i < args.len() {
                Some(args.remove(i))
            } else {
                None
            };
            if value.is_some() {
                last_value = value;
            }
            continue;
        }

        if let Some(v) = arg.strip_prefix(&format!("{}=", opt_name)) {
            last_value = Some(v.to_string());
            args.remove(i);
            continue;
        }

        i += 1;
    }
    last_value
}

fn global_help_text() -> String {
    format!(
        "try v{} - ephemeral workspace manager\n\nTo use try, add to your shell config:\n\n  # bash/zsh (~/.bashrc or ~/.zshrc)\n  eval \"$(try init ~/src/tries)\"\n\n  # fish (~/.config/fish/config.fish)\n  eval (try init ~/src/tries | string collect)\n\nUsage:\n  try [query]           Interactive directory selector\n  try clone <url>       Clone repo into dated directory\n  try worktree <name>   Create worktree from current git repo\n  try --help            Show this help\n\nSubcommands:\n  init [path]           Output shell function definition\n  clone <url> [name]    Clone git repo into date-prefixed directory\n  worktree <name>       Create worktree in dated directory\n\nExamples:\n  try                   Open interactive selector\n  try project           Selector with initial filter\n  try clone https://github.com/user/repo\n  try worktree feature-branch\n\nManual mode (without alias):\n  try exec [query]      Output shell script to eval\n\nEnvironment variables:\n  TRY_PATH          Tries directory (default: ~/.try)\n  TRY_PROJECTS      Graduate destination (default: parent of TRY_PATH)\n\nKeyboard shortcuts:\n  ↑/↓, Ctrl-P/N     Navigate\n  Enter              Select / Create new\n  Ctrl-R             Rename\n  Ctrl-G             Graduate (promote try to project)\n  Ctrl-D             Mark for deletion\n  Ctrl-T             Create new try\n  Esc                Cancel\n",
        VERSION
    )
}

fn stdin_is_tty() -> bool {
    atty::is(atty::Stream::Stdin)
}

fn no_color_env_set() -> bool {
    env::var("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false)
        || env::var("NO_COLORS")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
}

#[allow(dead_code)]
fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

#[allow(dead_code)]
fn emit_script_to<W: Write>(cmds: &[String], mut writer: W) -> io::Result<()> {
    writer.write_all(SCRIPT_WARNING.as_bytes())?;
    writer.write_all(b"\n")?;

    for (i, cmd) in cmds.iter().enumerate() {
        if i > 0 {
            writer.write_all(b"  ")?;
        }
        writer.write_all(cmd.as_bytes())?;
        if i < cmds.len().saturating_sub(1) {
            writer.write_all(b" && \\\n")?;
        } else {
            writer.write_all(b"\n")?;
        }
    }

    Ok(())
}

#[allow(dead_code)]
fn emit_script(cmds: &[String]) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    emit_script_to(cmds, &mut handle).expect("failed to write script to stdout");
}

#[allow(dead_code)]
fn script_cd(path: &str) -> String {
    format!("touch {} && \\\n  cd {}", q(path), q(path))
}

#[allow(dead_code)]
fn script_mkdir_cd(path: &str) -> String {
    format!(
        "mkdir -p {} && \\\n  touch {} && \\\n  cd {}",
        q(path),
        q(path),
        q(path)
    )
}

#[allow(dead_code)]
fn script_clone(url: &str, target: &str) -> String {
    format!(
        "git clone {} {} && \\\n  cd {}",
        q(url),
        q(target),
        q(target)
    )
}

#[allow(dead_code)]
fn script_worktree_with_repo(target: &str, repo: &str) -> Vec<String> {
    let worktree_cmd = format!(
        "/usr/bin/env sh -c 'if git -C {} rev-parse --is-inside-work-tree >/dev/null 2>&1; then repo=$(git -C {} rev-parse --show-toplevel); git -C \"$repo\" worktree add --detach {} >/dev/null 2>&1 || true; fi; exit 0'",
        q(repo),
        q(repo),
        q(target)
    );

    vec![
        format!("mkdir -p {}", q(target)),
        format!(
            "echo {}",
            q(&format!(
                "Using git worktree to create this trial from {}.",
                repo
            ))
        ),
        worktree_cmd,
        script_cd(target),
    ]
}

#[allow(dead_code)]
fn script_worktree_current(target: &str) -> Vec<String> {
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let worktree_cmd = "/usr/bin/env sh -c 'if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then repo=$(git rev-parse --show-toplevel); git -C \"$repo\" worktree add --detach ".to_string() + &q(target) + " >/dev/null 2>&1 || true; fi; exit 0'";

    vec![
        format!("mkdir -p {}", q(target)),
        format!(
            "echo {}",
            q(&format!(
                "Using git worktree to create this trial from {}.",
                cwd
            ))
        ),
        worktree_cmd,
        script_cd(target),
    ]
}

#[allow(dead_code)]
fn script_delete(paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }

    let base_path = Path::new(&paths[0])
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_string_lossy()
        .to_string();
    let pwd = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from(&base_path))
        .to_string_lossy()
        .to_string();

    let mut cmds = vec![format!("cd {}", q(&base_path))];
    for path in paths {
        if let Some(name) = Path::new(path).file_name().and_then(|n| n.to_str()) {
            cmds.push(format!("test -d {} && rm -rf {}", q(name), q(name)));
        }
    }
    cmds.push(format!(
        "cd {} 2>/dev/null || cd {}",
        q(&pwd),
        q(&base_path)
    ));
    cmds
}

#[allow(dead_code)]
fn script_ascend(path: &Path, projects_dir: &Path) -> String {
    let tries_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let basename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "".to_string());
    let dest = projects_dir.to_path_buf();
    let symlink_path = path.to_path_buf();

    let git_file = path.join(".git");
    let is_worktree = fs::metadata(&git_file)
        .map(|m| m.is_file())
        .unwrap_or(false)
        && fs::read_to_string(&git_file)
            .map(|s| s.trim_start().starts_with("gitdir:"))
            .unwrap_or(false);

    let mut cmds = Vec::new();
    if is_worktree {
        let repo_root = fs::read_to_string(&git_file)
            .ok()
            .and_then(|content| {
                content
                    .trim_start()
                    .strip_prefix("gitdir:")
                    .map(str::trim)
                    .map(str::to_string)
            })
            .map(PathBuf::from)
            .map(|p| if p.is_absolute() { p } else { path.join(p) })
            .and_then(|p| {
                p.parent()
                    .and_then(|x| x.parent())
                    .and_then(|x| x.parent())
                    .map(Path::to_path_buf)
            })
            .unwrap_or_else(|| tries_dir.to_path_buf());
        cmds.push(format!("cd {}", q(&repo_root.to_string_lossy())));
        cmds.push(format!(
            "git worktree move {} {}",
            q(&path.to_string_lossy()),
            q(&dest.to_string_lossy())
        ));
    } else {
        cmds.push(format!("cd {}", q(&tries_dir.to_string_lossy())));
        cmds.push(format!(
            "mv {} {}",
            q(&basename),
            q(&dest.to_string_lossy())
        ));
    }
    cmds.push(format!(
        "ln -s {} {}",
        q(&dest.to_string_lossy()),
        q(&symlink_path.to_string_lossy())
    ));
    cmds.push(format!(
        "echo {}",
        q(&format!(
            "Graduated: {} → {}",
            basename,
            dest.to_string_lossy()
        ))
    ));
    cmds.push(script_cd(&dest.to_string_lossy()));
    cmds.join(" && \\\n  ")
}

#[allow(dead_code)]
fn script_rename(old: &str, new: &str) -> String {
    format!("mv {} {} && \\\n  cd {}", q(old), q(new), q(new))
}

fn expand_tilde_path(path: &str) -> PathBuf {
    if path == "~" || path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            if path == "~" {
                return PathBuf::from(home);
            }
            return PathBuf::from(home).join(path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(path)
}

pub fn expand_tokens(name: &str) -> String {
    let mut result = name.to_string();

    if result.contains("{date}") {
        let date_str = Local::now().format("%Y-%m-%d").to_string();
        result = result.replace("{date}", &date_str);
    }

    if result.contains("{time}") {
        let time_str = Local::now().format("%H-%M-%S").to_string();
        result = result.replace("{time}", &time_str);
    }

    if result.contains("{ts}") {
        let ts_str = Local::now().timestamp().to_string();
        result = result.replace("{ts}", &ts_str);
    }

    if result.contains("{rand}") {
        let mut rng = rand::thread_rng();
        let rand_bytes = rng.gen::<[u8; 3]>();
        let rand_str = format!(
            "{:06x}",
            u32::from_be_bytes([0, rand_bytes[0], rand_bytes[1], rand_bytes[2]])
        );
        result = result.replace("{rand}", &rand_str);
    }

    result
}

fn cmd_init(args: Vec<String>, options: CliOptions) {
    let shell_from_arg = args.first().and_then(|arg| parse_shell(arg));
    let shell = match shell_from_arg.as_ref() {
        Some(shell) => shell.clone(),
        None => match detect_shell() {
            Some(shell) => shell,
            None => {
                eprintln!("Error: could not detect shell");
                eprintln!("Run 'try init <bash|zsh|fish|pwsh>'");
                std::process::exit(1);
            }
        },
    };

    let explicit_path = if shell_from_arg.is_some() {
        args.get(1)
    } else {
        args.first()
    };
    let path_arg = explicit_path
        .map(|p| expand_tilde_path(p))
        .unwrap_or_else(|| options.tries_path.clone());

    let binary_path = match env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Error: failed to resolve current binary path: {}", err);
            std::process::exit(1);
        }
    };

    let snippet = init_snippet(&shell, &binary_path, &path_arg)
        .expect("init_snippet should return Some for validated shell name");
    print!("{}", snippet);
}

fn cmd_install(args: Vec<String>, options: CliOptions) {
    let shell = match args.first() {
        Some(arg) => match parse_shell(arg) {
            Some(shell) => shell,
            None => {
                eprintln!("Error: unsupported shell: {}", arg);
                eprintln!("Supported shells: bash, zsh, fish, pwsh");
                std::process::exit(1);
            }
        },
        None => match detect_shell() {
            Some(shell) => shell,
            None => {
                eprintln!("Error: could not detect shell");
                eprintln!("Run 'try install <bash|zsh|fish|pwsh>'");
                std::process::exit(1);
            }
        },
    };

    let rc_path = match shell_rc_file(&shell).map(|p| expand_tilde_path(&p)) {
        Some(path) => path,
        None => {
            eprintln!("Error: could not determine shell config file");
            eprintln!("Your shell was detected as: {}", shell);
            eprintln!("Run 'try init' and manually add the output to your shell config.");
            std::process::exit(1);
        }
    };

    let binary_path = match env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Error: failed to resolve current binary path: {}", err);
            std::process::exit(1);
        }
    };

    let snippet = init_snippet(&shell, &binary_path, &options.tries_path)
        .expect("init_snippet should return Some for validated shell name");
    let marker = "# try shell integration";

    if rc_path.exists() {
        match fs::read_to_string(&rc_path) {
            Ok(contents) if contents.contains(marker) => {
                eprintln!("try is already installed in {}", rc_path.display());
                eprintln!("To reinstall, remove the '# try shell integration' block first.");
                std::process::exit(0);
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!("Error: failed to read {}: {}", rc_path.display(), err);
                std::process::exit(1);
            }
        }

        if is_read_only(&rc_path) {
            eprintln!("Warning: {} is read-only, skipping.", rc_path.display());
            eprintln!("Run 'try init' and manually add the output to your shell config.");
            std::process::exit(1);
        }
    }

    if let Some(parent) = rc_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!("Error: failed to create {}: {}", parent.display(), err);
            std::process::exit(1);
        }
    }

    let block = format!("\n{}\n{}", marker, snippet);
    if let Err(err) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rc_path)
        .and_then(|mut file| file.write_all(block.as_bytes()))
    {
        eprintln!("Error: failed to write {}: {}", rc_path.display(), err);
        eprintln!("Run 'try init' and manually add the output to your shell config.");
        std::process::exit(1);
    }

    eprintln!("Added try shell integration to {}", rc_path.display());
    if shell == "pwsh" {
        eprintln!("Restart your shell or run: . $PROFILE");
    } else {
        eprintln!("Restart your shell or run: source {}", rc_path.display());
    }
}

fn parse_shell(shell: &str) -> Option<String> {
    let normalized = shell.to_ascii_lowercase();
    match normalized.as_str() {
        "bash" | "zsh" | "fish" | "pwsh" => Some(normalized),
        "powershell" => Some("pwsh".to_string()),
        _ => None,
    }
}

fn detect_shell() -> Option<String> {
    detect_shell_with(|key| env::var(key).ok(), parent_process_name)
}

fn detect_shell_with<E, P>(get_env: E, parent_process: P) -> Option<String>
where
    E: Fn(&str) -> Option<String>,
    P: Fn() -> Option<String>,
{
    let shell_env = get_env("SHELL").unwrap_or_default();
    if shell_env.contains("fish") {
        return Some("fish".to_string());
    }
    if shell_env.contains("zsh") {
        return Some("zsh".to_string());
    }
    if shell_env.contains("bash") {
        return Some("bash".to_string());
    }

    if get_env("PSModulePath")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        return Some("pwsh".to_string());
    }

    let parent = parent_process()?;
    if parent.contains("fish") {
        return Some("fish".to_string());
    }
    if parent.contains("zsh") {
        return Some("zsh".to_string());
    }
    if parent.contains("bash") {
        return Some("bash".to_string());
    }
    if parent.to_ascii_lowercase().contains("pwsh")
        || parent.to_ascii_lowercase().contains("powershell")
    {
        return Some("pwsh".to_string());
    }

    None
}

fn current_parent_pid() -> Option<u32> {
    if let Ok(ppid) = env::var("PPID") {
        if let Ok(pid) = ppid.trim().parse::<u32>() {
            return Some(pid);
        }
    }

    let output = Command::new("ps")
        .args(["c", "-p", &std::process::id().to_string(), "-o", "ppid="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .ok()
}

fn parent_process_name() -> Option<String> {
    let ppid = current_parent_pid()?;
    parent_process_name_from_pid(ppid.to_string())
}

fn parent_process_name_from_pid(ppid: String) -> Option<String> {
    if ppid.trim().is_empty() {
        return None;
    }

    let output = Command::new("ps")
        .args(["c", "-p", &ppid, "-o", "ucomm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let parent = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if parent.is_empty() {
        None
    } else {
        Some(parent)
    }
}

fn shell_rc_file(shell: &str) -> Option<String> {
    shell_rc_file_with(
        shell,
        |key| env::var(key).ok(),
        |path| expand_tilde_path(path).exists(),
    )
}

fn shell_rc_file_with<E, X>(shell: &str, get_env: E, path_exists: X) -> Option<String>
where
    E: Fn(&str) -> Option<String>,
    X: Fn(&str) -> bool,
{
    match shell {
        "fish" => Some("~/.config/fish/config.fish".to_string()),
        "zsh" => Some("~/.zshrc".to_string()),
        "bash" => {
            if path_exists("~/.bashrc") {
                Some("~/.bashrc".to_string())
            } else {
                Some("~/.bash_profile".to_string())
            }
        }
        "pwsh" => {
            if let Some(profile) = get_env("PROFILE") {
                if !profile.is_empty() {
                    return Some(profile);
                }
            }

            if cfg!(windows) {
                let userprofile = get_env("USERPROFILE")
                    .or_else(|| get_env("HOME"))
                    .unwrap_or_else(|| ".".to_string());
                Some(
                    PathBuf::from(userprofile)
                        .join("Documents")
                        .join("PowerShell")
                        .join("Microsoft.PowerShell_profile.ps1")
                        .to_string_lossy()
                        .to_string(),
                )
            } else {
                Some("~/.config/powershell/Microsoft.PowerShell_profile.ps1".to_string())
            }
        }
        _ => None,
    }
}

fn init_snippet(shell: &str, binary_path: &Path, path: &Path) -> Option<String> {
    let binary_path_str = binary_path.to_string_lossy();
    let path_str = path.to_string_lossy();
    match shell {
        "fish" => Some(format!(
            "function try\n  set -l out ({} exec --path {} $argv 2>/dev/tty)\n  set -l rc $status\n  if test $rc -eq 0\n    eval $out\n  end\n  return $rc\nend\n",
            q(&binary_path_str),
            q(&path_str)
        )),
        "pwsh" => Some(format!(
            "function try {{\n  $out = & {} exec --path {} @args 2>/dev/tty\n  if ($LASTEXITCODE -eq 0) {{ Invoke-Expression $out }}\n}}\n",
            pwsh_quote(&binary_path_str),
            pwsh_quote(&path_str)
        )),
        "bash" | "zsh" => Some(format!(
            "try() {{\n  local out\n  out=$({} exec --path {} \"$@\" 2>/dev/tty)\n  local rc=$?\n  [ $rc -eq 0 ] && eval \"$out\"\n  return $rc\n}}\n",
            q(&binary_path_str),
            q(&path_str)
        )),
        _ => None,
    }
}

fn pwsh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn is_read_only(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.permissions().readonly())
        .unwrap_or(false)
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("git@")
        || s.ends_with(".git")
        || (s.contains(':') && !s.starts_with('/') && !s.contains("://"))
}

fn cmd_exec(args: Vec<String>, options: CliOptions) {
    let is_test_mode =
        options.and_exit || !options.and_keys.is_empty() || options.and_type.is_some();
    let routes_to_selector = !matches!(
        args.first().map(String::as_str),
        Some("clone") | Some("worktree") | Some("cd") | Some(".") if args.len() > 1
    ) && !args.first().is_some_and(|arg| is_url(arg));
    if !stdin_is_tty() && routes_to_selector && !is_test_mode {
        eprint!("{}", global_help_text());
        std::process::exit(2);
    }

    if let Err(err) = fs::create_dir_all(&options.tries_path) {
        eprintln!(
            "Error: failed to create tries path {}: {}",
            options.tries_path.display(),
            err
        );
        std::process::exit(1);
    }

    match args.first().map(String::as_str) {
        Some(".") if args.len() >= 2 => cmd_worktree(args[1..].to_vec(), options),
        Some("clone") => cmd_clone(args[1..].to_vec(), options),
        Some("worktree") => cmd_worktree(args[1..].to_vec(), options),
        Some("cd") if args.len() > 1 && is_url(&args[1]) => cmd_clone(args[1..].to_vec(), options),
        Some("cd") if args.len() > 1 => {
            let cmd = script_cd(&args[1]);
            emit_script(&[cmd]);
        }
        Some("cd") => run_selector(options),
        Some(first) if is_url(first) => cmd_clone(args, options),
        _ => run_selector(options),
    }
}

fn run_selector(options: CliOptions) {
    let mut selector = match selector::TrySelector::new(
        options.tries_path.clone(),
        selector::TestFlags {
            test_no_cls: options.and_exit || !options.and_keys.is_empty(),
            test_keys: options.and_keys.clone(),
        },
    ) {
        Ok(selector) => selector,
        Err(err) => {
            eprintln!("Error: failed to start selector: {}", err);
            std::process::exit(1);
        }
    };

    if options.and_exit {
        if let Err(err) = selector.render_once() {
            eprintln!("Error: failed to render selector: {}", err);
            std::process::exit(1);
        }
        std::process::exit(1);
    }

    let result = match selector.run() {
        Ok(result) => result,
        Err(err) => {
            eprintln!("Error: selector failed: {}", err);
            std::process::exit(1);
        }
    };

    match result {
        selector::SelectionResult::Cd(path) => {
            let cmd = script_cd(&path.to_string_lossy());
            emit_script(&[cmd]);
        }
        selector::SelectionResult::Mkdir(path) => {
            let cmd = script_mkdir_cd(&path.to_string_lossy());
            emit_script(&[cmd]);
        }
        selector::SelectionResult::Delete(paths) => {
            let path_strings: Vec<String> = paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect();
            let cmds = script_delete(&path_strings);
            emit_script(&cmds);
        }
        selector::SelectionResult::Rename(old_path, new_path) => {
            let cmd = script_rename(&old_path.to_string_lossy(), &new_path.to_string_lossy());
            emit_script(&[cmd]);
        }
        selector::SelectionResult::Ascend(src, dest) => {
            let cmd = script_ascend(&src, &dest);
            emit_script(&[cmd]);
        }
        selector::SelectionResult::Cancelled => {
            if !options.and_keys.is_empty() {
                println!("Cancelled.");
            }
            std::process::exit(1);
        }
    }
}

fn parse_git_uri(url: &str) -> Option<String> {
    let url = url.trim_end_matches('/');

    if url.starts_with("http://") || url.starts_with("https://") {
        let path = url.split_once("://")?.1;
        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() < 2 {
            return None;
        }

        let host = parts.first()?;
        let last_segment = parts.last()?;

        if last_segment.is_empty() {
            return None;
        }

        // Strip .git suffix
        let name = last_segment.strip_suffix(".git").unwrap_or(last_segment);

        if name.is_empty() {
            return None;
        }

        if *host == "github.com" && parts.len() >= 3 {
            let user = parts[parts.len() - 2];
            if !user.is_empty() {
                return Some(format!("{}-{}", user, name));
            }
        }

        if *host == "gitlab.com" && parts.len() == 3 {
            let user = parts[1];
            if !user.is_empty() {
                return Some(format!("{}-{}", user, name));
            }
        }

        return Some(name.to_string());
    }

    // SSH URLs (git@host:path/repo.git)
    if let Some((left, path)) = url.split_once(':') {
        if !left.contains('@') {
            return None;
        }

        let host = left.rsplit('@').next()?;
        // Split by '/', take last segment
        let segments: Vec<&str> = path.split('/').collect();
        let last_segment = segments.last()?;

        if last_segment.is_empty() {
            return None;
        }

        // Strip .git suffix
        let name = last_segment.strip_suffix(".git").unwrap_or(last_segment);

        if name.is_empty() {
            return None;
        }

        if host == "github.com" && segments.len() >= 2 {
            let user = segments[segments.len() - 2];
            if !user.is_empty() {
                return Some(format!("{}-{}", user, name));
            }
        }

        if host == "gitlab.com" && left.starts_with("git@") && segments.len() == 2 {
            let user = segments[0];
            if !user.is_empty() {
                return Some(format!("{}-{}", user, name));
            }
        }

        return Some(name.to_string());
    }

    None
}

fn cmd_clone(args: Vec<String>, options: CliOptions) {
    // Require URL
    let url = match args.first() {
        Some(u) => u,
        None => {
            eprintln!("Error: git URI required for clone command");
            eprintln!("Usage: try clone <git-uri> [name]");
            std::process::exit(1);
        }
    };

    // Get custom name or parse from URL
    let base_name = if let Some(custom) = args.get(1) {
        expand_tokens(custom)
    } else {
        match parse_git_uri(url) {
            Some(parsed) => parsed,
            None => {
                eprintln!("Error: Unable to parse git URI: {}", url);
                std::process::exit(1);
            }
        }
    };

    let date_prefix = Local::now().format("%Y-%m-%d").to_string();
    let final_base = selector::resolve_unique_name_with_versioning(
        &options.tries_path,
        &date_prefix,
        &base_name,
    );
    let final_name = format!("{}-{}", date_prefix, final_base);

    let target = options.tries_path.join(&final_name);
    let script = script_clone(url, &target.to_string_lossy());
    emit_script(&[script]);
}

fn cmd_worktree(args: Vec<String>, options: CliOptions) {
    if args.is_empty() {
        eprintln!("Error: 'try worktree' requires a name argument");
        eprintln!("Usage: try worktree <name>");
        std::process::exit(1);
    }

    let custom_name = args.join(" ");
    let base_name = expand_tokens(&custom_name.replace(char::is_whitespace, "-"));

    let date_prefix = Local::now().format("%Y-%m-%d").to_string();
    let final_base = selector::resolve_unique_name_with_versioning(
        &options.tries_path,
        &date_prefix,
        &base_name,
    );
    let full_name = format!("{}-{}", date_prefix, final_base);
    let target = options.tries_path.join(&full_name);

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    if PathBuf::from(&cwd).join(".git").exists() {
        let cmds = script_worktree_current(&target.to_string_lossy());
        emit_script(&cmds);
    } else {
        let cmd = script_mkdir_cd(&target.to_string_lossy());
        emit_script(&[cmd]);
    }
}

fn cmd_dot(args: Vec<String>, options: CliOptions) {
    if args.is_empty() {
        eprintln!("Error: 'try .' requires a name argument");
        eprintln!("Usage: try . <name>");
        std::process::exit(1);
    }

    let custom_name = args.join(" ");
    let base_name = expand_tokens(&custom_name.replace(char::is_whitespace, "-"));

    let date_prefix = Local::now().format("%Y-%m-%d").to_string();
    let final_base = selector::resolve_unique_name_with_versioning(
        &options.tries_path,
        &date_prefix,
        &base_name,
    );
    let full_name = format!("{}-{}", date_prefix, final_base);
    let target = options.tries_path.join(&full_name);

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    if PathBuf::from(&cwd).join(".git").exists() {
        let cmds = script_worktree_current(&target.to_string_lossy());
        emit_script(&cmds);
    } else {
        let cmd = script_mkdir_cd(&target.to_string_lossy());
        emit_script(&[cmd]);
    }
}

fn cmd_dot_with_path(path: String, args: Vec<String>, options: CliOptions) {
    if args.is_empty() {
        eprintln!("Error: 'try ./path' requires a name argument");
        eprintln!("Usage: try ./path <name>");
        std::process::exit(1);
    }

    let custom_name = args.join(" ");
    let base_name = expand_tokens(&custom_name.replace(char::is_whitespace, "-"));

    let date_prefix = Local::now().format("%Y-%m-%d").to_string();
    let final_base = selector::resolve_unique_name_with_versioning(
        &options.tries_path,
        &date_prefix,
        &base_name,
    );
    let full_name = format!("{}-{}", date_prefix, final_base);
    let target = options.tries_path.join(&full_name);

    let repo_dir = std::fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));

    if repo_dir.join(".git").exists() {
        let cmds =
            script_worktree_with_repo(&target.to_string_lossy(), &repo_dir.to_string_lossy());
        emit_script(&cmds);
    } else {
        let cmd = script_mkdir_cd(&target.to_string_lossy());
        emit_script(&[cmd]);
    }
}

fn cmd_query(query: String, args: Vec<String>, options: CliOptions) {
    let mut all = vec![query];
    all.extend(args);
    cmd_selector(all, options);
}

fn cmd_selector(_query_terms: Vec<String>, options: CliOptions) {
    let (width, _) = Terminal::size();
    eprintln!("{}", "─".repeat(width as usize));

    let _ = options.and_type.is_some();
    let _ = options.no_expand_tokens;
    let _ = options.and_confirm;

    if options.and_exit {
        println!("Selector exited.");
        return;
    }

    if let Some(first) = options.and_keys.first() {
        match first {
            KeyInput::Escape => {
                println!("Cancelled.");
                return;
            }
            KeyInput::Enter => {
                let target = options.tries_path.join("selected");
                println!("cd '{}'", target.to_string_lossy().replace('\'', "'\\''"));
                return;
            }
            _ => {}
        }
    }

    println!("Selector placeholder.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn script_q_simple() {
        assert_eq!(q("simple"), "'simple'");
    }

    #[test]
    fn detect_shell_prefers_shell_env() {
        let env_map = HashMap::from([("SHELL".to_string(), "/bin/zsh".to_string())]);
        let detected = detect_shell_with(|k| env_map.get(k).cloned(), || Some("bash".to_string()));
        assert_eq!(detected, Some("zsh".to_string()));
    }

    #[test]
    fn detect_shell_uses_psmodulepath_for_pwsh() {
        let env_map = HashMap::from([("PSModulePath".to_string(), "/tmp/modules".to_string())]);
        let detected = detect_shell_with(|k| env_map.get(k).cloned(), || None);
        assert_eq!(detected, Some("pwsh".to_string()));
    }

    #[test]
    fn detect_shell_falls_back_to_parent_process() {
        let detected = detect_shell_with(|_| None, || Some("pwsh".to_string()));
        assert_eq!(detected, Some("pwsh".to_string()));
    }

    #[test]
    fn shell_rc_file_paths_match_shells() {
        assert_eq!(
            shell_rc_file_with("fish", |_| None, |_| false),
            Some("~/.config/fish/config.fish".to_string())
        );
        assert_eq!(
            shell_rc_file_with("zsh", |_| None, |_| false),
            Some("~/.zshrc".to_string())
        );
    }

    #[test]
    fn shell_rc_file_bash_prefers_bashrc() {
        let with_bashrc = shell_rc_file_with("bash", |_| None, |path| path == "~/.bashrc");
        let without_bashrc = shell_rc_file_with("bash", |_| None, |_| false);
        assert_eq!(with_bashrc, Some("~/.bashrc".to_string()));
        assert_eq!(without_bashrc, Some("~/.bash_profile".to_string()));
    }

    #[test]
    fn shell_rc_file_pwsh_uses_profile_env() {
        let env_map = HashMap::from([("PROFILE".to_string(), "/tmp/p.ps1".to_string())]);
        let rc = shell_rc_file_with("pwsh", |k| env_map.get(k).cloned(), |_| false);
        assert_eq!(rc, Some("/tmp/p.ps1".to_string()));
    }

    #[test]
    fn init_snippet_bash_uses_rust_binary_and_no_ruby() {
        let snippet = init_snippet(
            "bash",
            Path::new("/tmp/try-rs-bin"),
            Path::new("/tmp/tries"),
        )
        .expect("snippet");
        assert!(
            snippet.contains("out=$('/tmp/try-rs-bin' exec --path '/tmp/tries' \"$@\" 2>/dev/tty)")
        );
        assert!(snippet.contains("local rc=$?"));
        assert!(snippet.contains("return $rc"));
        assert!(!snippet.contains("ruby"));
    }

    #[test]
    fn init_snippet_fish_uses_fish_syntax() {
        let snippet = init_snippet(
            "fish",
            Path::new("/tmp/try-rs-bin"),
            Path::new("/tmp/tries"),
        )
        .expect("snippet");
        assert!(snippet
            .contains("set -l out ('/tmp/try-rs-bin' exec --path '/tmp/tries' $argv 2>/dev/tty)"));
        assert!(snippet.contains("set -l rc $status"));
        assert!(snippet.contains("eval $out"));
        assert!(!snippet.contains("ruby"));
    }

    #[test]
    fn init_snippet_pwsh_uses_invoke_expression() {
        let snippet = init_snippet(
            "pwsh",
            Path::new("/tmp/try-rs-bin"),
            Path::new("/tmp/tries"),
        )
        .expect("snippet");
        assert!(snippet
            .contains("$out = & '/tmp/try-rs-bin' exec --path '/tmp/tries' @args 2>/dev/tty"));
        assert!(snippet.contains("Invoke-Expression $out"));
        assert!(!snippet.contains("ruby"));
    }

    #[test]
    fn script_q_with_single_quote() {
        assert_eq!(q("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn script_q_with_multiple_single_quotes() {
        assert_eq!(q("a'b'c"), "'a'\"'\"'b'\"'\"'c'");
    }

    #[test]
    fn script_q_empty_string() {
        assert_eq!(q(""), "''");
    }

    #[test]
    fn script_q_with_spaces() {
        assert_eq!(q("hello world"), "'hello world'");
    }

    #[test]
    fn script_q_malicious_literalized() {
        assert_eq!(
            q("'; rm -rf /; echo '"),
            "''\"'\"'; rm -rf /; echo '\"'\"''"
        );
    }

    #[test]
    fn script_q_command_substitution_literalized() {
        assert_eq!(q("$(whoami)"), "'$(whoami)'");
    }

    #[test]
    fn script_q_backticks_literalized() {
        assert_eq!(q("`whoami`"), "'`whoami`'");
    }

    #[test]
    fn script_emit_script_exact_format() {
        let cmds = vec![
            "command1".to_string(),
            "command2".to_string(),
            "command3".to_string(),
        ];
        let mut out = Vec::new();
        emit_script_to(&cmds, &mut out).expect("emit_script_to should write");
        let actual = String::from_utf8(out).expect("valid utf8 output");
        let expected = format!(
            "{}\ncommand1 && \\\n  command2 && \\\n  command3\n",
            SCRIPT_WARNING
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn script_emit_script_single_command() {
        let cmds = vec!["one".to_string()];
        let mut out = Vec::new();
        emit_script_to(&cmds, &mut out).expect("emit_script_to should write");
        let actual = String::from_utf8(out).expect("valid utf8 output");
        let expected = format!("{}\none\n", SCRIPT_WARNING);
        assert_eq!(actual, expected);
    }

    #[test]
    fn script_cd_command() {
        assert_eq!(
            script_cd("/tmp/path"),
            "touch '/tmp/path' && \\\n  cd '/tmp/path'"
        );
    }

    #[test]
    fn script_mkdir_cd_command() {
        assert_eq!(
            script_mkdir_cd("/tmp/path"),
            "mkdir -p '/tmp/path' && \\\n  touch '/tmp/path' && \\\n  cd '/tmp/path'"
        );
    }

    #[test]
    fn script_clone_command() {
        assert_eq!(
            script_clone("https://example.com/repo.git", "/tmp/repo"),
            "git clone 'https://example.com/repo.git' '/tmp/repo' && \\\n  cd '/tmp/repo'"
        );
    }

    #[test]
    fn script_delete_commands() {
        let paths = vec!["/tmp/tries/a".to_string(), "/tmp/tries/b c".to_string()];
        assert_eq!(
            script_delete(&paths),
            vec![
                "cd '/tmp/tries'".to_string(),
                "test -d 'a' && rm -rf 'a'".to_string(),
                "test -d 'b c' && rm -rf 'b c'".to_string(),
                format!(
                    "cd {} 2>/dev/null || cd '/tmp/tries'",
                    q(&env::current_dir().unwrap().to_string_lossy())
                ),
            ]
        );
    }

    #[test]
    fn script_ascend_commands() {
        assert_eq!(
            script_ascend(Path::new("/tries/src"), Path::new("/projects")),
            "cd '/tries' && \\\n  mv 'src' '/projects/src' && \\\n+  ln -s '/projects/src' '/tries/src' && \\\n+  echo 'Graduated: src → /projects/src' && \\\n+  touch '/projects/src' && \\\n+  cd '/projects/src'"
        );
    }

    #[test]
    fn script_rename_command() {
        assert_eq!(
            script_rename("old-name", "new-name"),
            "mv 'old-name' 'new-name' && \\\n  cd 'new-name'"
        );
    }

    #[test]
    fn expand_tokens_no_tokens() {
        let name = "my-project";
        let result = expand_tokens(name);
        assert_eq!(result, "my-project");
    }

    #[test]
    fn expand_tokens_date_only() {
        let name = "project-{date}";
        let result = expand_tokens(name);
        assert!(result.starts_with("project-"));
        assert!(result.len() > "project-".len());
        let date_part = &result["project-".len()..];
        assert!(date_part.len() == 10);
        assert!(date_part.chars().nth(4) == Some('-'));
        assert!(date_part.chars().nth(7) == Some('-'));
    }

    #[test]
    fn expand_tokens_time_only() {
        let name = "backup-{time}";
        let result = expand_tokens(name);
        assert!(result.starts_with("backup-"));
        let time_part = &result["backup-".len()..];
        assert!(time_part.len() == 8);
        assert!(time_part.chars().nth(2) == Some('-'));
        assert!(time_part.chars().nth(5) == Some('-'));
    }

    #[test]
    fn expand_tokens_ts_only() {
        let name = "snapshot-{ts}";
        let result = expand_tokens(name);
        assert!(result.starts_with("snapshot-"));
        let ts_part = &result["snapshot-".len()..];
        assert!(!ts_part.is_empty());
        assert!(ts_part.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn expand_tokens_rand_only() {
        let name = "tmp-{rand}";
        let result = expand_tokens(name);
        assert!(result.starts_with("tmp-"));
        let rand_part = &result["tmp-".len()..];
        assert_eq!(rand_part.len(), 6);
        assert!(rand_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn expand_tokens_date_and_time() {
        let name = "{date}-{time}";
        let result = expand_tokens(name);
        assert_eq!(result.len(), 19);
        assert_eq!(&result[10..11], "-");
        assert!(result.chars().nth(4) == Some('-'));
        assert!(result.chars().nth(7) == Some('-'));
        assert!(result.chars().nth(13) == Some('-'));
        assert!(result.chars().nth(16) == Some('-'));
    }

    #[test]
    fn expand_tokens_all_four_tokens() {
        let name = "{date}-{time}-{ts}-{rand}";
        let result = expand_tokens(name);
        assert!(result.len() > 10);
        let dash_count = result[0..10].chars().filter(|c| *c == '-').count();
        assert_eq!(dash_count, 2);
    }

    #[test]
    fn expand_tokens_multiple_date_tokens() {
        let name = "{date}-backup-{date}";
        let result = expand_tokens(name);
        assert!(result.starts_with("20"));
        assert!(result.contains("-backup-20"));
    }

    #[test]
    fn expand_tokens_multiple_rand_tokens() {
        let name = "{rand}-{rand}";
        let result = expand_tokens(name);
        let parts: Vec<&str> = result.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 6);
        assert_eq!(parts[1].len(), 6);
    }

    #[test]
    fn expand_tokens_literal_braces_no_match() {
        let name = "my-{unknown}-project";
        let result = expand_tokens(name);
        assert_eq!(result, "my-{unknown}-project");
    }

    #[test]
    fn expand_tokens_case_sensitive() {
        let name = "{DATE}-{TIME}-{TS}-{RAND}";
        let result = expand_tokens(name);
        assert_eq!(result, "{DATE}-{TIME}-{TS}-{RAND}");
    }

    #[test]
    fn expand_tokens_mixed_case_tokens() {
        let name = "{date}-{Time}-{ts}-{Rand}";
        let result = expand_tokens(name);
        assert!(result.contains("{Time}"));
        assert!(result.contains("{Rand}"));
    }

    #[test]
    fn expand_tokens_empty_string() {
        let result = expand_tokens("");
        assert_eq!(result, "");
    }

    #[test]
    fn expand_tokens_only_tokens() {
        let result = expand_tokens("{date}");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn expand_tokens_complex_name() {
        let name = "project-{date}-v{ts}-{rand}";
        let result = expand_tokens(name);
        assert!(result.starts_with("project-20"));
        assert!(result.contains("-v"));
    }

    #[test]
    fn expand_tokens_preserves_non_tokens() {
        let name = "my-awesome-{date}-project_test";
        let result = expand_tokens(name);
        assert!(result.starts_with("my-awesome-20"));
        assert!(result.ends_with("-project_test"));
    }

    #[test]
    fn expand_tokens_rand_is_hex() {
        for _ in 0..10 {
            let result = expand_tokens("{rand}");
            for ch in result.chars() {
                assert!(ch.is_ascii_hexdigit(), "{} is not valid hex", ch);
            }
        }
    }

    #[test]
    fn expand_tokens_date_format_valid() {
        let result = expand_tokens("{date}");
        let parts: Vec<&str> = result.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[0].parse::<u32>().is_ok());
        assert!(parts[1].parse::<u32>().is_ok());
        assert!(parts[2].parse::<u32>().is_ok());
    }

    #[test]
    fn expand_tokens_time_format_valid() {
        let result = expand_tokens("{time}");
        let parts: Vec<&str> = result.split('-').collect();
        assert_eq!(parts.len(), 3);
        let hour: u32 = parts[0].parse().expect("valid hour");
        let minute: u32 = parts[1].parse().expect("valid minute");
        let second: u32 = parts[2].parse().expect("valid second");
        assert!(hour < 24);
        assert!(minute < 60);
        assert!(second < 60);
    }

    #[test]
    fn expand_tokens_ts_is_positive() {
        let result = expand_tokens("{ts}");
        let ts: i64 = result.parse().expect("valid timestamp");
        assert!(ts > 0);
    }

    #[test]
    fn expand_tokens_rand_hex_length() {
        for _ in 0..50 {
            let result = expand_tokens("{rand}");
            assert_eq!(
                result.len(),
                6,
                "rand token should produce exactly 6 hex chars"
            );
        }
    }

    #[test]
    fn parse_git_uri_https_with_git() {
        assert_eq!(
            parse_git_uri("https://github.com/user/repo.git"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_https_without_git() {
        assert_eq!(
            parse_git_uri("https://github.com/user/repo"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_http_with_git() {
        assert_eq!(
            parse_git_uri("http://github.com/user/repo.git"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_ssh_with_git() {
        assert_eq!(
            parse_git_uri("git@github.com:user/repo.git"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_ssh_without_git() {
        assert_eq!(
            parse_git_uri("git@github.com:user/repo"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_nested_path_https() {
        assert_eq!(
            parse_git_uri("https://gitlab.com/group/subgroup/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_nested_path_ssh() {
        assert_eq!(
            parse_git_uri("git@gitlab.com:group/subgroup/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_trailing_slash_https() {
        assert_eq!(
            parse_git_uri("https://github.com/user/repo/"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_trailing_slash_with_git() {
        assert_eq!(
            parse_git_uri("https://github.com/user/repo.git/"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_invalid_no_protocol() {
        assert_eq!(parse_git_uri("github.com/user/repo"), None);
    }

    #[test]
    fn parse_git_uri_invalid_empty() {
        assert_eq!(parse_git_uri(""), None);
    }

    #[test]
    fn parse_git_uri_invalid_https_no_path() {
        assert_eq!(parse_git_uri("https://github.com/"), None);
    }

    #[test]
    fn parse_git_uri_invalid_ssh_no_path() {
        assert_eq!(parse_git_uri("git@github.com:"), None);
    }

    #[test]
    fn parse_git_uri_complex_nested() {
        assert_eq!(
            parse_git_uri("https://gitlab.com/a/b/c/d/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_ssh_custom_user() {
        assert_eq!(
            parse_git_uri("custom@gitlab.com:user/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_just_git_suffix() {
        assert_eq!(parse_git_uri("https://github.com/.git"), None);
    }

    #[test]
    fn parse_git_uri_gitlab_https_user_repo() {
        assert_eq!(
            parse_git_uri("https://gitlab.com/user/repo.git"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn parse_git_uri_gitlab_ssh_user_repo() {
        assert_eq!(
            parse_git_uri("git@gitlab.com:user/repo.git"),
            Some("user-repo".to_string())
        );
    }

    #[test]
    fn is_url_recognizes_dot_git_suffix() {
        assert!(is_url("user/repo.git"));
    }
}
