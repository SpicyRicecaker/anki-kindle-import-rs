//! Any highlight is treated as an [Clipping::Highlight]
//! Any note is treated as a [Clipping::Note], with [Clipping::Note::terms] counted per newline
//! Except:
//! - ` .. ` can be added after a term per line, which makes content after the ` ..` functions as "extra" content (thoughts, ideas, etc.). Additional newlines can be added via addtional ` .. `
//! - ` ...` can be added after term to designate the word type as cloze. After which, any content after the ` ... ` functions as "extra" content. Please note that there can only be one cloze term per sentence as of now.

use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::prelude::*;
use chrono::serde::ts_seconds;
use clap::{arg, Command};
use env_logger::Env;
use log::info;
use regex::Regex;
use serde::{Deserialize, Serialize};

use anyhow::{bail, Context, Error};
use chrono::{TimeZone, Utc};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
enum Clipping {
    Highlight {
        book: String,
        author: String,
        #[serde(with = "ts_seconds")]
        date: DateTime<Utc>,
        sentence: String,
    },
    Note {
        book: String,
        author: String,
        date: DateTime<Utc>,
        terms: Vec<Term>,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Term {
    term: String,
    definition: String,
}

enum OutputFileFormat {
    Yaml,
    Json,
}

impl OutputFileFormat {
    pub fn get_extension(&self) -> &str {
        match self {
            OutputFileFormat::Yaml => "yml",
            OutputFileFormat::Json => "json",
        }
    }
}

struct Config {
    output_file_name: String,
    output_file_format: OutputFileFormat,
}

impl Config {
    pub fn new(output_file_format: OutputFileFormat) -> Config {
        let output_file_name = format!("output.{}", output_file_format.get_extension());
        Config {
            output_file_name,
            output_file_format,
        }
    }
}

fn main() -> Result<(), Error> {
    // initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("trace")).init();
    info!("Program started");

    // set output file format
    let config = Config::new(OutputFileFormat::Yaml);

    // create clap app
    let matches = Command::new("anki-kindle-import")
        .version("0.1.0")
        .author("Andy Li <SpicyRicecaker@gmail.com>")
        .about("Turns kindle clippings into structure easily parsible by Anki")
        .arg(arg!(--validate "check the output file to make sure there is one highlight per one note, then compiles it"))
        .arg(arg!(-s --start-date "only include clippings from the start date, inclusive"))
        .arg(arg!(--clipping-path "the path to the kindle clippings"))
        .get_matches();

    // check if we should validate, and continue on with the rest of the program
    if matches.is_present("validate") {
        info!("program started with validate flag, now validating...");
        validate(&config)?;
        info!("successfullly validated program");
        std::process::exit(0);
    }


    // get optional argument if needed
    let date_inclusive_after = if let Some(date_string) = matches.value_of("start-date") {
        info!("program started with value of start date");
        Some(date_from_str(date_string)?)
    } else {
        None
    };

    // get clipping path & reading clipping
    let clippings_path = if matches.is_present("clipping-path") {
        PathBuf::from(matches.value_of("clipping-path").unwrap())
    } else {
        let mut t = dirs::home_dir().unwrap();
        t.push(r"Calibre Library/Kindle/My Clippings (13)/My Clippings - Kindle.txt");
        t
    };
    let clippings_txt =
        fs::read_to_string(clippings_path).with_context(|| "unable to read clippings path")?;

    // store all entries
    let mut entries = Vec::new();

    let re_author_book = Regex::new(r"(?P<book>.+) \((?P<author>.+)\)").unwrap();
    let re_date = Regex::new(
        // r"- Your (?P<highlight_or_note>.+) on page \d+ \| .+ \| Added on .+, (?P<date>.+,.+)",
        r"- Your (?P<highlight_or_note>.+) on .+ (\| .+ )?\| Added on .+, (?P<date>.+,.+)",
    )?;

    let mut iter = clippings_txt.lines();
    while let Some(line_1) = iter.next() {
        // first line is always the book and author
        let (book, author) = {
            let captures = re_author_book.captures(line_1).unwrap();
            (captures["book"].to_string(), captures["author"].to_string())
        };
        let line_2 = iter.next().unwrap();
        let (highlight_or_note, date) = {
            let captures = re_date.captures(line_2).unwrap();
            (
                captures["highlight_or_note"].to_string(),
                captures["date"].to_string(),
            )
        };
        // e.g. November 24, 2018 11:31:30 AM
        let naive = NaiveDateTime::parse_from_str(&date, "%B %d, %Y %-I:%M:%S %p").unwrap();
        let date: DateTime<Utc> = Local.from_local_datetime(&naive).unwrap().into();

        // always two newlines
        iter.next().unwrap();

        match highlight_or_note.as_str() {
            "Highlight" => {
                let mut content = Vec::new();
                // grab everything until the next `======`
                for line in iter.by_ref() {
                    if line.contains("==========") {
                        break;
                    }
                    content.push(line);
                }
                let sentence = content.join("\n");
                entries.push(Clipping::Highlight {
                    book,
                    author,
                    date,
                    sentence,
                });
            }
            "Note" => {
                let mut terms = Vec::new();
                // grab everything until the next `======`
                for line in iter.by_ref() {
                    if line.contains("==========") {
                        break;
                    }
                    terms.push(Term {
                        term: line.to_string().to_lowercase(),
                        definition: String::new(),
                    });
                }
                entries.push(Clipping::Note {
                    book,
                    author,
                    date,
                    terms,
                });
            }
            "Bookmark" => {
                // fast forward and consume until either EOF or the next `=========`
                for line in iter.by_ref() {
                    if line.contains("==========") {
                        break;
                    }
                }
            }
            _ => {
                panic!("unexpected type of kindle annotation");
            }
        };
        // next line is always (notesorhighlight | location | date)
    }
    if let Some(date_inclusive_after) = date_inclusive_after {
        entries = entries
            .into_iter()
            .filter(|c| match c {
                Clipping::Highlight { date, .. } => date >= &date_inclusive_after,
                Clipping::Note { date, .. } => date >= &date_inclusive_after,
            })
            .collect();
    }

    // check if file already exists
    let out_path = Path::new(&config.output_file_name);
    if out_path.exists() {
        // copy file to `out.json (old)`
        let copy = format!("output-copy.{}", config.output_file_format.get_extension());
        fs::copy(out_path, &copy).with_context(|| {
            format!(
                "unable to copy from {:#?} to {:#?} for some reason",
                out_path, copy
            )
        })?;
        println!("overwrote old {:?} (backed up to `{:?}`)", out_path, copy);
    }
    // copy to something
    fs::write(
        &config.output_file_name,
        match config.output_file_format {
            OutputFileFormat::Json => serde_json::to_string(&entries).unwrap(),
            OutputFileFormat::Yaml => serde_yaml::to_string(&entries).unwrap(),
        },
    )
    .with_context(|| "Unable to write to final output file `out.json/yml` for some reason.")?;
    Ok(())
}

fn date_from_str(date_str: &str) -> Result<DateTime<Utc>, Error> {
    let naive_time = NaiveTime::from_hms(0, 0, 0);
    let naive_date = NaiveDate::parse_from_str(date_str, "%m-%d-%Y")?;
    let naive_date_time = NaiveDateTime::new(naive_date, naive_time);
    Ok(Local.from_local_datetime(&naive_date_time).unwrap().into())
}

/// Validates if notes have one highlight and one (or more) terms. This doesn't
/// work for cloze types, so it's kinda useless rn
fn validate(config: &Config) -> Result<(), Error> {
    // parse the file in to JSON
    let vec: Vec<Clipping> = match config.output_file_format {
        OutputFileFormat::Yaml => {
            serde_yaml::from_str(&std::fs::read_to_string(&config.output_file_name).unwrap())
                .unwrap()
        }
        OutputFileFormat::Json => {
            serde_json::from_str(&std::fs::read_to_string(&config.output_file_name).unwrap())
                .unwrap()
        }
    };

    // make sure that there is one highlight per one note (currently doesn't take into account cloze)
    vec.chunks(2).enumerate().try_for_each(|(idx, arr)| {
        if arr.len() != 2 {
            bail!(
                "unable to form pair {idx} since their lengths don't match {:#?} {:#?}",
                arr[0],
                arr[1]
            )
        }

        if let Clipping::Note { .. } = arr[0] {
            bail!(
                "expected highlight at pair {idx}, found note: {:#?}",
                arr[0]
            )
        }
        if let Clipping::Highlight { .. } = arr[1] {
            bail!(
                "expected note at pair {idx}, found highlight: {:#?}",
                arr[1]
            )
        }
        Ok(())
    })?;

    // attempt to compile the file
    
    fs::write(
        "output.json",
            serde_json::to_string(&vec).unwrap(),
    )
    .with_context(|| "Unable to write to final output file `out.json` for some reason.")?;

    Ok(())
}
