use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

// ── Tool definitions ────────────────────────────────────────────────────────

struct ToolDef {
    name: &'static str,
    /// args that go BEFORE the user's question
    ask_args: &'static [&'static str],
}

const TOOL_DEFS: &[ToolDef] = &[
    ToolDef { name: "claude",  ask_args: &["-p"] },
    ToolDef { name: "codex",   ask_args: &[] },
    ToolDef { name: "gemini",  ask_args: &["-p"] },
    ToolDef { name: "ollama",  ask_args: &["run", "llama3"] },
    ToolDef { name: "llm",     ask_args: &[] },
    ToolDef { name: "aider",   ask_args: &["--message"] },
    ToolDef { name: "kiro",    ask_args: &[] },
    ToolDef { name: "cody",    ask_args: &[] },
    ToolDef { name: "copilot", ask_args: &["suggest"] },
    ToolDef { name: "gpt",     ask_args: &[] },
];

// ── Found tool ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct InstalledTool {
    name: String,
    path: PathBuf,
    version: String,
    ask_args: Vec<String>,
}

// ── Scanner ─────────────────────────────────────────────────────────────────

fn extra_search_paths() -> Vec<PathBuf> {
    let mut paths = vec![];

    if let Some(home) = home_dir() {
        paths.push(home.join(".cargo").join("bin"));
        paths.push(home.join(".npm-global").join("bin"));
        paths.push(home.join(".local").join("bin"));
    }

    // Windows npm global
    if let Ok(appdata) = env::var("APPDATA") {
        paths.push(PathBuf::from(&appdata).join("npm"));
    }

    // Homebrew (Mac)
    paths.push(PathBuf::from("/usr/local/bin"));
    paths.push(PathBuf::from("/opt/homebrew/bin"));

    paths
}

fn home_dir() -> Option<PathBuf> {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .ok()
        .map(PathBuf::from)
}

/// Return the full path of a tool executable if found in `dir`.
/// On Windows also checks .cmd and .exe variants.
fn find_exe(dir: &PathBuf, name: &str) -> Option<PathBuf> {
    if cfg!(windows) {
        for ext in &["", ".exe", ".cmd", ".bat", ".ps1"] {
            let p = dir.join(format!("{}{}", name, ext));
            if p.exists() {
                return Some(p);
            }
        }
    } else {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn get_version(name: &str) -> String {
    Command::new(name)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            let combined = if out.is_empty() { err } else { out };
            if combined.is_empty() { None } else { Some(combined) }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn scan() -> Vec<InstalledTool> {
    let path_var = env::var("PATH").unwrap_or_default();
    let sep = if cfg!(windows) { ";" } else { ":" };

    let mut dirs: Vec<PathBuf> = path_var.split(sep).map(PathBuf::from).collect();
    dirs.extend(extra_search_paths());

    // Dedup while preserving order
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));

    let mut found = vec![];
    let mut found_names = std::collections::HashSet::new();

    for def in TOOL_DEFS {
        if found_names.contains(def.name) {
            continue;
        }
        for dir in &dirs {
            if let Some(path) = find_exe(dir, def.name) {
                let version = get_version(def.name);
                found.push(InstalledTool {
                    name: def.name.to_string(),
                    path,
                    version,
                    ask_args: def.ask_args.iter().map(|s| s.to_string()).collect(),
                });
                found_names.insert(def.name);
                break;
            }
        }
    }

    found
}

// ── Ask ─────────────────────────────────────────────────────────────────────

fn ask(tool: &InstalledTool, question: &str) -> Result<String, String> {
    let mut cmd = Command::new(&tool.name);

    for arg in &tool.ask_args {
        cmd.arg(arg);
    }
    cmd.arg(question);

    let output = cmd.output().map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && stdout.trim().is_empty() {
        return Err(if stderr.is_empty() {
            format!("tool exited with code {:?}", output.status.code())
        } else {
            stderr
        });
    }

    Ok(if stdout.trim().is_empty() { stderr } else { stdout })
}

// ── UI helpers ───────────────────────────────────────────────────────────────

fn prompt(label: &str) -> String {
    print!("{}", label);
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    buf.trim().to_string()
}

fn print_table(tools: &[InstalledTool]) {
    println!("\n{:<4} {:<10} {:<45} {}", "#", "Tool", "Path", "Version");
    println!("{}", "─".repeat(75));
    for (i, t) in tools.iter().enumerate() {
        println!(
            "{:<4} {:<10} {:<45} {}",
            i + 1,
            t.name,
            t.path.display(),
            t.version
        );
    }
    println!();
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    println!("Scanning system for LLM CLI tools...\n");

    let tools = scan();

    if tools.is_empty() {
        println!("No known LLM CLI tools found on your system.");
        return;
    }

    println!("Found {} tool(s):", tools.len());
    print_table(&tools);

    // Tool selection
    let selected_index = loop {
        let input = prompt("Select a tool by number: ");
        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= tools.len() => break n - 1,
            _ => println!("Invalid choice. Enter a number between 1 and {}.", tools.len()),
        }
    };

    let selected = &tools[selected_index];
    println!("\nUsing: {} ({})\n", selected.name, selected.version);
    println!("Type your question and press Enter. Type 'exit' to quit.\n");

    // Question loop
    loop {
        let question = prompt("You: ");

        if question.eq_ignore_ascii_case("exit") || question.eq_ignore_ascii_case("quit") {
            println!("Bye!");
            break;
        }

        if question.is_empty() {
            continue;
        }

        println!("\n{} →\n", selected.name);

        match ask(selected, &question) {
            Ok(response) => println!("{}\n", response),
            Err(e) => println!("[Error] {}\n", e),
        }
    }
}
