use clap::Parser;
use crossterm::{
    style::{Color, Stylize},
    terminal,
};
use glob::glob;
use regex::Regex;
use serde::Deserialize;
use std::{
    cmp::max,
    collections::HashMap,
    fs::{self, Metadata},
    path::{Path, PathBuf, MAIN_SEPARATOR},
    process,
};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[derive(Deserialize, Debug)]
struct OptionalConfig {
    aliases: Option<HashMap<String, String>>,
    folders: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
    colors: Option<HashMap<String, String>>,
    ignore: Option<HashMap<String, Vec<String>>>,
}

#[derive(Deserialize, Debug)]
struct Config {
    aliases: HashMap<String, String>,
    folders: HashMap<String, String>,
    files: HashMap<String, String>,
    colors: HashMap<String, String>,
    ignore: HashMap<String, Vec<String>>,
}

#[derive(Debug)]
struct Entry {
    path: PathBuf,
    metadata: Option<Metadata>,
}

#[derive(Error, Debug)]
enum Error {
    #[error("unable to retrieve metadata for {0:?}")]
    Metadata(PathBuf),

    #[error("unable to retrieve the home directory")]
    HomeDirNotFound,

    #[error("couldn't find the specified path {0:?}")]
    PathNotFound(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Glob(#[from] glob::PatternError),
}

/// A simple implementation of the `ls` command that uses
/// [NerdFonts](https://www.nerdfonts.com/) and colored output by default.
///
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Cmd {
    /// The entry that must be displayed.
    /// Can also be a glob pattern.
    path: Option<String>,

    /// Force output to be one entry per line.
    #[arg(short = '1')]
    single_column: bool,

    /// Show all files and folders, disabling the `ignore` configuration.
    #[arg(long, short = 'a')]
    all: bool,
}

fn main() {
    match run() {
        Ok(()) => (),
        Err(error) => {
            eprintln!("ERROR: {error}");
            process::exit(1);
        }
    }
}

fn run() -> Result<(), Error> {
    let config = get_config()?;
    let cmd = Cmd::parse();
    let mut input = expand_path(
        &cmd.path
            .clone()
            .unwrap_or(format!(".{MAIN_SEPARATOR}*").to_string()),
    );

    if let Ok(metadata) = fs::metadata(input.clone()) {
        if metadata.is_dir() {
            input = Path::new(&input).join("*").to_str().unwrap().to_string();
        }
    }

    let paths: Vec<PathBuf> = glob(input.clone().as_str())?
        .filter_map(Result::ok)
        .collect();

    let parent = Path::new(&input)
        .parent()
        .expect("Couldn't get the parent dir");

    if let Ok(basedir) = fs::canonicalize(parent) {
        show_entries(&cmd, &config, &paths, &basedir);
        return Ok(());
    }

    Err(Error::PathNotFound(input))
}

fn get_color_from_string(color_name: &str) -> Color {
    match color_name.to_lowercase().as_str() {
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "grey" => Color::Grey,
        "darkred" => Color::DarkRed,
        "darkgreen" => Color::DarkGreen,
        "darkyellow" => Color::DarkYellow,
        "darkblue" => Color::DarkBlue,
        "darkmagenta" => Color::DarkMagenta,
        "darkcyan" => Color::DarkCyan,
        "darkgrey" => Color::DarkGrey,
        _ => Color::Black,
    }
}

fn format_with_color(config: &Config, message: String, name: &str) -> String {
    match supports_color::on(supports_color::Stream::Stdout) {
        Some(_) => {
            let default_color = "black".to_string();
            let color_name = config.colors.get(name).unwrap_or(&default_color);

            message.with(get_color_from_string(color_name)).to_string()
        }
        _ => message,
    }
}

fn resolve_icon(
    icons: &HashMap<String, String>,
    aliases: &HashMap<String, String>,
    fallback: &str,
    queries: Vec<String>,
) -> String {
    let mut icon = fallback.to_string();

    for query in queries {
        if let Some(value) = icons.get(&query) {
            icon = value.to_string();
            break;
        }
    }

    // Resolve aliases (only one level).
    if let Some(found) = aliases.get(&icon) {
        icon = found.clone();
    }

    icon
}

fn build_file_entry(config: &Config, metadata: &fs::Metadata, path: &Path, _pwd: &Path) -> String {
    let dirname = path
        .parent()
        .expect("couldn't find parent dir")
        .to_str()
        .unwrap_or_default()
        .to_string();
    let basename = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap()
        .to_string();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap()
        .to_lowercase();
    let ext = format!(".{ext}");

    let icon = resolve_icon(
        &config.files,
        &config.aliases,
        "\u{ea7b}",
        vec![
            format!("{dirname}/{basename}"),
            basename.clone(),
            ext,
            "file".to_string(),
        ],
    );

    let size = bytesize::ByteSize::b(get_file_size(metadata))
        .to_string()
        .replace(' ', "");

    let color_type = if is_executable(path, metadata) {
        "executable_file"
    } else if basename.starts_with('.') {
        "hidden"
    } else {
        "file"
    };

    let mut input = format_with_color(config, format!("  {icon} {basename}"), color_type);

    input = format!(
        "{input} {}",
        format_with_color(config, size.to_string(), "file_size")
    );

    input
}

fn build_dir_entry(config: &Config, _metadata: &fs::Metadata, path: &Path) -> String {
    let basename = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap()
        .to_string();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap()
        .to_lowercase();
    let ext = format!(".{ext}");

    let icon = resolve_icon(
        &config.folders,
        &config.aliases,
        "\u{e5ff}",
        vec![basename.clone(), ext, "folder".to_string()],
    );

    let color_type = if basename.starts_with('.') {
        "hidden_dir"
    } else {
        "dir"
    };

    let input = format!("  {icon} {basename}/");

    format_with_color(config, input, color_type)
}

fn show_entries(cmd: &Cmd, config: &Config, paths: &[PathBuf], pwd: &PathBuf) {
    let folders: Vec<String> = config
        .ignore
        .get("folders")
        .expect("Couldn't get ignore.folders")
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    let files: Vec<String> = config
        .ignore
        .get("files")
        .expect("Couldn't get ignore.files")
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    let mut entries: Vec<Entry> = paths
        .iter()
        .map(|path| Entry {
            path: path.clone(),
            metadata: fs::metadata(path.clone())
                .map_err(|_| Error::Metadata(path.clone()))
                .ok(),
        })
        .filter(|entry| !entry.path.display().to_string().ends_with('.'))
        .filter(|entry| cmd.all || ignore_entry(entry, &folders, &files))
        .collect();

    entries.sort_by_key(|entry| {
        entry
            .path
            .file_name()
            .map(|name| name.to_os_string().to_ascii_lowercase())
    });

    let mut list: Vec<String> = vec![];

    for entry in entries {
        let relative_path = pathdiff::diff_paths(&entry.path, pwd).unwrap_or(entry.path.clone());

        let Some(metadata) = entry.metadata else {
            let item = format_with_color(
                config,
                format!("  \u{f481} {}", relative_path.display()),
                "dead_link",
            );

            list.push(item);

            continue;
        };

        let item = if metadata.is_dir() {
            build_dir_entry(config, &metadata, &relative_path)
        } else {
            build_file_entry(config, &metadata, &relative_path, pwd)
        };

        list.push(item);
    }

    if cmd.single_column {
        for item in list {
            println!("{item}");
        }
    } else {
        display_in_columns(&list);
    }
}

fn get_config_file() -> Result<PathBuf, Error> {
    let config_dir = if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(&config_home)
    } else {
        dirs::home_dir()
            .ok_or(Error::HomeDirNotFound)?
            .join(".config")
    };

    Ok(config_dir.join("ll.toml"))
}

fn get_config() -> Result<Config, Error> {
    let toml_str = include_str!("config.toml");
    let mut config: Config = toml::from_str(toml_str).expect("Failed to parse TOML file");
    let config_file = get_config_file()?;

    if config_file.exists() {
        let toml_str = fs::read_to_string(&config_file)?;
        let custom_config: OptionalConfig =
            toml::from_str(&toml_str).expect("Failed to parse TOML file");

        config
            .folders
            .extend(custom_config.folders.unwrap_or_default());

        config.files.extend(custom_config.files.unwrap_or_default());

        config
            .colors
            .extend(custom_config.colors.unwrap_or_default());

        config
            .aliases
            .extend(custom_config.aliases.unwrap_or_default());

        let ignore = custom_config.ignore.unwrap_or_default();

        if let Some(files) = ignore.get("files") {
            config.ignore.insert("files".to_string(), files.clone());
        }

        if let Some(folders) = ignore.get("folders") {
            config.ignore.insert("folders".to_string(), folders.clone());
        }
    }

    Ok(config)
}

fn display_in_columns(list: &[String]) {
    let max_item_len = list
        .iter()
        .map(|i| visible_length(i))
        .max()
        .unwrap_or_default();

    let term_width: usize = if let Ok((width, _)) = terminal::size() {
        width.into()
    } else {
        1
    };

    let list_len = list.len();
    let col_gap = 2;
    let col_width = max_item_len + col_gap;
    let mut cols = max(1, term_width / col_width);

    // similar result to using .ceil() on a floating-point division
    let mut rows = max(1, (list_len + cols - 1) / cols);

    if rows == 1 {
        cols = 1;
        rows = list_len;
    }

    for row in 0..rows {
        for col in 0..cols {
            let index = col * rows + row;

            if index < list_len {
                let value = &list[index];
                let value_len = visible_length(value);
                let padding = " ".repeat(col_width - value_len);

                print!("{value}{padding}");
            }
        }

        println!();
    }
}

fn visible_length(input: &str) -> usize {
    let ansi_escape = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    let stripped = ansi_escape.replace_all(input, "");

    stripped.chars().count()
}

#[cfg(unix)]
fn is_executable(_path: &Path, metadata: &Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(windows)]
fn is_executable(path: &Path, _metadata: &Metadata) -> bool {
    if let Some(ext) = path.extension() {
        matches!(ext.to_str(), Some("exe" | "bat" | "cmd"))
    } else {
        false
    }
}

fn get_file_size(metadata: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        metadata.size()
    }

    #[cfg(windows)]
    {
        metadata.file_size()
    }
}

fn ignore_entry(entry: &Entry, folders: &[String], files: &[String]) -> bool {
    let basename = entry
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        .to_lowercase();

    let extname = entry
        .path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        .to_lowercase();
    let extname = format!(".{extname}");

    // If we're able to retrieve the metadata, be specific about the type of
    // entry and its ignored values; otherwise, compare the file name against
    // everything.
    if let Some(metadata) = &entry.metadata {
        if metadata.is_dir() {
            return !folders.contains(&basename);
        }

        return !(files.contains(&basename) || files.contains(&extname));
    };

    !(files.contains(&basename) || files.contains(&extname) || folders.contains(&basename))
}

fn expand_path(input: &str) -> String {
    let mut path = input;

    if let Some(stripped) = input.strip_prefix("~") {
        path = stripped;

        if let Some(stripped) = path.strip_prefix("/") {
            path = stripped;
        }

        let home_dir = dirs::home_dir().expect("Couldn't find home directory");
        return home_dir.join(path).to_string_lossy().to_string();
    }

    path.to_string()
}
