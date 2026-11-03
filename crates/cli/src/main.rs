use crate::args::{CliArgs, CliSubcommand};
use crate::logger::setup_logger;
use brarchive::SerializeOptions;
use clap::Parser;
use log::{error, info, warn};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Instant;

mod args;
mod logger;

/// Name of the archive under `__brarchive/` that holds the files from the
/// pack's top-level directory. A fixed name (rather than one derived from the
/// pack directory) lets `decode --recursive` put them back at the root no
/// matter what the pack has been renamed to.
const ROOT_ARCHIVE: &str = "__root__.brarchive";

fn main() {
    let args = CliArgs::parse();
    setup_logger(args.log_path);

    match args.command {
        CliSubcommand::Encode {
            path,
            out,
            recursive,
            dedup,
            delete_source,
            skip_root,
        } => {
            let start_time = Instant::now();

            if recursive {
                let out_base = out.unwrap_or_else(|| path.clone());
                let archive_root = out_base.join("__brarchive");
                let opts = EncodeOptions {
                    dedup,
                    delete_source,
                    skip_root,
                };
                encode_recursive(&path, &path, &archive_root, opts);
                info!(
                    "Successfully encoded recursively in {}!",
                    humantime::format_duration(start_time.elapsed())
                );
            } else {
                let out = out.unwrap_or_else(|| {
                    extract_file_name(&path).unwrap_or(PathBuf::from("brarchive"))
                });
                let out = add_extension_if_missing(out, "brarchive");
                encode_single(&path, &out, dedup, delete_source);
                info!(
                    "Successfully encoded archive in {}!",
                    humantime::format_duration(start_time.elapsed())
                );
            }
        }
        CliSubcommand::List { path, recursive } => {
            if recursive {
                let archive_root = path.join("__brarchive");
                if !archive_root.exists() {
                    error!("No __brarchive/ directory found in \"{}\"", path.display());
                    exit(1);
                }
                list_recursive(&archive_root, &archive_root);
            } else {
                list_single(&path);
            }
        }
        CliSubcommand::Decode {
            path,
            out,
            recursive,
            delete_source,
            pretty,
            overwrite,
        } => {
            let start_time = Instant::now();
            let opts = DecodeOptions {
                delete_source,
                pretty,
                overwrite,
            };

            if recursive {
                let archive_root = path.join("__brarchive");
                if !archive_root.exists() {
                    error!("No __brarchive/ directory found in \"{}\"", path.display());
                    exit(1);
                }
                let out_base = out.unwrap_or_else(|| path.clone());
                decode_recursive(&archive_root, &archive_root, &out_base, opts);
                info!(
                    "Successfully decoded recursively in {}!",
                    humantime::format_duration(start_time.elapsed())
                );
            } else {
                let out = out.unwrap_or_else(|| {
                    extract_file_name(&path).unwrap_or(PathBuf::from("brarchive"))
                });
                decode_single(&path, &out, opts);
                info!(
                    "Successfully decoded archive in {}!",
                    humantime::format_duration(start_time.elapsed())
                );
            }
        }
    }
}

fn encode_single(path: &Path, out: &Path, dedup: bool, delete_source: bool) {
    if !path.exists() {
        error!("Input \"{}\" does not exist", path.display());
        exit(1);
    }
    if out.exists() {
        error!("Output \"{}\" already exists", out.display());
        exit(1);
    }

    let entries_map: BTreeMap<String, Vec<u8>> = if path.is_dir() {
        let read_dir = fs::read_dir(path).unwrap_or_else(|err| {
            error!("Failed to read directory \"{}\": {}", path.display(), err);
            exit(1);
        });
        let mut map = BTreeMap::new();
        for entry in read_dir {
            let entry = entry.unwrap_or_else(|err| {
                error!("{}", err);
                exit(1);
            });
            if !entry.path().is_file() {
                continue;
            }
            let content = fs::read(entry.path()).unwrap_or_else(|err| {
                error!("Failed to read \"{}\": {}", entry.path().display(), err);
                exit(1);
            });
            let name = entry
                .path()
                .strip_prefix(path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            map.insert(name, content);
        }
        map
    } else {
        let content = fs::read(path).unwrap_or_else(|err| {
            error!("Failed to read \"{}\": {}", path.display(), err);
            exit(1);
        });
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap()
            .to_string();
        BTreeMap::from([(name, content)])
    };

    let archive = brarchive::serialize_with(entries_map, SerializeOptions { dedup })
        .unwrap_or_else(|err| {
            error!("Failed to encode: {}", err);
            exit(1);
        });

    fs::write(out, &archive).unwrap_or_else(|err| {
        error!("Failed to write \"{}\": {}", out.display(), err);
        exit(1);
    });

    if delete_source {
        if path.is_dir() {
            fs::remove_dir_all(path).unwrap_or_else(|err| {
                error!("Failed to delete source \"{}\": {}", path.display(), err);
            });
        } else {
            fs::remove_file(path).unwrap_or_else(|err| {
                error!("Failed to delete source \"{}\": {}", path.display(), err);
            });
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EncodeOptions {
    dedup: bool,
    delete_source: bool,
    skip_root: bool,
}

/// Encode every directory under `source_root` into its own archive below
/// `archive_root`, mirroring the directory tree. Files in `source_root`
/// itself go to `__root__.brarchive` unless `skip_root` is set.
fn encode_recursive(source_root: &Path, current: &Path, archive_root: &Path, opts: EncodeOptions) {
    let is_root = current == source_root;
    let read_dir = fs::read_dir(current).unwrap_or_else(|err| {
        error!("Failed to read \"{}\": {}", current.display(), err);
        exit(1);
    });

    let mut subdirs = Vec::new();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    for entry in read_dir {
        let entry = entry.unwrap_or_else(|err| {
            error!("{}", err);
            exit(1);
        });
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().and_then(OsStr::to_str) != Some("__brarchive") {
                subdirs.push(p);
            }
        } else if p.is_file() {
            let content = match fs::read(&p) {
                Ok(c) => c,
                Err(err) => {
                    warn!("Skipping unreadable file \"{}\": {}", p.display(), err);
                    continue;
                }
            };
            let name = p.file_name().and_then(OsStr::to_str).unwrap().to_string();
            files.insert(name, content);
        }
    }

    if is_root && opts.skip_root {
        files.clear();
    }

    if !files.is_empty() {
        let archive_path = if is_root {
            archive_root.join(ROOT_ARCHIVE)
        } else {
            let relative = current.strip_prefix(source_root).unwrap_or(Path::new(""));
            add_extension_if_missing(archive_root.join(relative), "brarchive")
        };

        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| {
                error!("Failed to create directory: {}", err);
                exit(1);
            });
        }

        let archive = brarchive::serialize_with(&files, SerializeOptions { dedup: opts.dedup })
            .unwrap_or_else(|err| {
                error!("Failed to encode: {}", err);
                exit(1);
            });

        fs::write(&archive_path, &archive).unwrap_or_else(|err| {
            error!("Failed to write \"{}\": {}", archive_path.display(), err);
            exit(1);
        });

        info!("Encoded \"{}\"", archive_path.display());

        if opts.delete_source {
            for name in files.keys() {
                let file_path = current.join(name);
                fs::remove_file(&file_path).unwrap_or_else(|err| {
                    error!("Failed to delete \"{}\": {}", file_path.display(), err);
                });
            }
        }
    }

    for subdir in subdirs {
        encode_recursive(source_root, &subdir, archive_root, opts);
    }
}

#[derive(Debug, Clone, Copy)]
struct DecodeOptions {
    delete_source: bool,
    pretty: bool,
    overwrite: bool,
}

/// Decode one archive into `out`. The output directory is created if needed
/// and merged with whatever is already there, since a pack keeps loose files
/// (textures, sounds, ...) next to the JSON that lives in `__brarchive/`.
/// Existing files are never clobbered unless `--overwrite` is given.
fn decode_single(path: &Path, out: &Path, opts: DecodeOptions) {
    if !path.exists() {
        error!("Input \"{}\" does not exist", path.display());
        exit(1);
    }

    let data = fs::read(path).unwrap_or_else(|err| {
        error!("Failed to read \"{}\": {}", path.display(), err);
        exit(1);
    });

    let archive: BTreeMap<String, Vec<u8>> = brarchive::deserialize(&data).unwrap_or_else(|err| {
        error!("Failed to decode \"{}\": {}", path.display(), err);
        exit(1);
    });

    if out.exists() && !out.is_dir() {
        error!("Output \"{}\" exists and is not a directory", out.display());
        exit(1);
    }

    if !opts.overwrite {
        let existing: Vec<&String> = archive
            .keys()
            .filter(|file| out.join(file).exists())
            .collect();
        if !existing.is_empty() {
            error!(
                "Refusing to overwrite {} existing file(s) in \"{}\" while decoding \"{}\" (pass --overwrite to replace them):",
                existing.len(),
                out.display(),
                path.display()
            );
            for file in &existing {
                error!("  {}", out.join(file).display());
            }
            exit(1);
        }
    }

    fs::create_dir_all(out).unwrap_or_else(|err| {
        error!("Failed to create output directory: {}", err);
        exit(1);
    });

    for (file, contents) in archive {
        let dest = out.join(&file);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| {
                error!("Failed to create directories: {}", err);
                exit(1);
            });
        }
        let contents = if opts.pretty {
            prettify_json(contents)
        } else {
            contents
        };
        fs::write(&dest, contents).unwrap_or_else(|err| {
            error!("Failed to write \"{}\": {}", dest.display(), err);
        });
        info!("Decoded {:?}", file);
    }

    if opts.delete_source {
        fs::remove_file(path).unwrap_or_else(|err| {
            error!("Failed to delete source \"{}\": {}", path.display(), err);
        });
    }
}

/// Pretty-print `contents` if it parses as JSON (2-space indent, key order
/// preserved); otherwise return it untouched so binary entries such as compiled
/// MCB blobs stay byte-for-byte identical.
fn prettify_json(contents: Vec<u8>) -> Vec<u8> {
    match serde_json::from_slice::<serde_json::Value>(&contents) {
        Ok(value) => serde_json::to_vec_pretty(&value).unwrap_or(contents),
        Err(_) => contents,
    }
}

fn decode_recursive(archive_root: &Path, current: &Path, out_root: &Path, opts: DecodeOptions) {
    let read_dir = fs::read_dir(current).unwrap_or_else(|err| {
        error!("Failed to read \"{}\": {}", current.display(), err);
        exit(1);
    });

    for entry in read_dir {
        let entry = entry.unwrap_or_else(|err| {
            error!("{}", err);
            exit(1);
        });
        let p = entry.path();

        if p.is_dir() {
            decode_recursive(archive_root, &p, out_root, opts);
        } else if p.is_file() && p.extension().and_then(OsStr::to_str) == Some("brarchive") {
            let relative = p.strip_prefix(archive_root).unwrap_or(&p);
            let out_dir = if relative == Path::new(ROOT_ARCHIVE) {
                out_root.to_path_buf()
            } else {
                out_root.join(relative.with_extension(""))
            };
            if out_dir.starts_with(archive_root) {
                error!(
                    "Skipping \"{}\": output path \"{}\" would collide with archive root",
                    p.display(),
                    out_dir.display()
                );
                continue;
            }
            decode_single(&p, &out_dir, opts);
        }
    }
}

fn list_single(path: &Path) {
    let data = fs::read(path).unwrap_or_else(|err| {
        error!("Failed to read \"{}\": {}", path.display(), err);
        exit(1);
    });
    let entries = brarchive::entries(&data).unwrap_or_else(|err| {
        error!("Failed to list \"{}\": {}", path.display(), err);
        exit(1);
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            error!("Failed to list \"{}\": {}", path.display(), err);
            exit(1);
        });
        println!("{}", entry.name);
    }
}

fn list_recursive(archive_root: &Path, current: &Path) {
    let read_dir = fs::read_dir(current).unwrap_or_else(|err| {
        error!("Failed to read \"{}\": {}", current.display(), err);
        exit(1);
    });
    for entry in read_dir {
        let entry = entry.unwrap_or_else(|err| {
            error!("{}", err);
            exit(1);
        });
        let p = entry.path();
        if p.is_dir() {
            list_recursive(archive_root, &p);
        } else if p.is_file() && p.extension().and_then(OsStr::to_str) == Some("brarchive") {
            let relative = p.strip_prefix(archive_root).unwrap_or(&p);
            println!("{}:", relative.display());
            list_single(&p);
        }
    }
}

fn extract_file_name(path: &Path) -> Option<PathBuf> {
    path.file_stem().and_then(OsStr::to_str).map(PathBuf::from)
}

fn add_extension_if_missing(mut path: PathBuf, extension: &str) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    path
}
