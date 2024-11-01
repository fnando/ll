use clap::Parser;
use crossterm::{
    style::{Color, Stylize},
    terminal,
};
use regex::Regex;
use serde::Deserialize;
use std::{
    cmp::max,
    collections::HashMap,
    env,
    fs::{self, Metadata},
    path::{Path, PathBuf},
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
}

#[derive(Deserialize, Debug)]
struct Config {
    aliases: HashMap<String, String>,
    folders: HashMap<String, String>,
    files: HashMap<String, String>,
    colors: HashMap<String, String>,
}

#[derive(Error, Debug)]
enum Error {
    #[error("couldn't retrieve the current directory")]
    CurrentDir,

    #[error("unable to retrieve metadata for {0:?}")]
    Metadata(PathBuf),

    #[error("unable to retrieve the home directory")]
    HomeDirNotFound,

    #[error("invalid path {0:?}")]
    InvalidPath(String),

    #[error("unable to read dir: {0:?}")]
    UnableToReadDir(PathBuf),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A simple implementation of the `ls` command that uses
/// [NerdFonts](https://www.nerdfonts.com/) and colored output by default.
///
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Cmd {
    /// The entry that must be displayed.
    path: Option<String>,

    /// Force output to be one entry per line.
    #[arg(short = '1')]
    single_column: bool,
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
    let path = if let Some(ref path) = cmd.path {
        fs::canonicalize(path.clone()).map_err(|_| Error::InvalidPath(path.clone()))?
    } else {
        env::current_dir().map_err(|_| Error::CurrentDir)?
    };

    let metadata = fs::metadata(path.clone()).map_err(|_| Error::Metadata(path.clone()))?;

    if metadata.is_dir() {
        show_dir(&cmd, &config, &metadata, &path)?;
    } else {
        let output = build_file_entry(&config, &metadata, &path);
        println!("{output}");
    }

    Ok(())
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

fn build_file_entry(config: &Config, metadata: &fs::Metadata, path: &Path) -> String {
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

fn show_dir(
    cmd: &Cmd,
    config: &Config,
    _metadata: &fs::Metadata,
    path: &Path,
) -> Result<(), Error> {
    let mut paths: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|_| Error::UnableToReadDir(path.into()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap_or_default() != ".DS_Store")
        .collect();

    paths.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_os_string().to_ascii_lowercase())
    });

    let mut list: Vec<String> = vec![];

    for entry_path in paths {
        let Ok(metadata) = fs::metadata(entry_path.clone()) else {
            let item = format_with_color(
                config,
                format!(
                    "  \u{f481} {}",
                    entry_path
                        .file_name()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                ),
                "dead_link",
            );

            list.push(item);

            continue;
        };

        let item = if metadata.is_dir() {
            build_dir_entry(config, &metadata, &entry_path)
        } else {
            build_file_entry(config, &metadata, &entry_path)
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

    Ok(())
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
        0
    };

    let list_len = list.len();
    let col_gap = 2;
    let col_width = max_item_len + col_gap;
    // similar result to using .ceil() on a floating-point division
    let mut cols = max(1, (term_width + col_width - 1) / col_width);
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
