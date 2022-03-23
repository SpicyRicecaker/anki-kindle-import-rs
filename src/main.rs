//! This program takes in a `clipping.txt` file from a Kindle and parses it into an intermediate human-readable format useful for creating Anki cards, and allows for recompiling them into a json format easily fed into Anki.
//! 
//! ## Concepts
//! Every Anki card has a front and a back. A useful vocab card has a definition
//! on the front, word on the back, along with the example sentence. 
//! 
//! ```txt
//! the definition of apple
//! ====
//! apple
//! 
//! 'A' stands for apple
//! ```
//! 
//! To make creating Anki cards as streamlined as possible, `anki-kindle-import-rs` relies on the following two principles:
//! 1. every kindle highlight is treated as an example sentence 
//! 2. every line of every kindle note is treated as a term.
//! 
//! During [Card] creation, a term and [Clipping::Highlight::sentence] is added to the backside of each card as
//! an example sentence, and the front is left empty, which the user must consult a dictionary to add the card defintion for.
//! 
//! For example, say that you have highlighted the sentence,
//! ```txt
//! The cat walked over a hill
//! ```
//! 
//! and added the note
//! ```txt
//! hill
//! walk
//! ```
//! 
//! `anki-kindle-import-rs` parses the `clipping.txt` file, and creates the following cards after `cargo run`:
//! 
//! ```txt
//! (empty)
//! ======
//! hill
//! 
//! the cat walked over the hill
//! ```
//! 
//! ```txt
//! (empty)
//! ======
//! walk
//! 
//! the cat walked over the hill
//! ```
//! 
//! You can then put your preferred dictionary definition at the front of the card, then run `cargo run -- --validate` to generate the `.json` for the card, which you can then feed into Anki.
//! 
//! ## To attach additional info to cards,
//! - ` .. ` can be added in a note. Content after the ` ..` is added to the
//! Anki backside after the example sentence as extra content. Additional 
//! 
//! For example, the following note and higlight pair
//! 
//! ```txt
//! the cat walked over the hill
//! ```
//! 
//! ```txt
//! hill .. I remember when I was six years old on a hill in yellowstone and almost rolled face-first into a pile of bison dung
//! ```
//! 
//! Would create the following card
//! 
//! ```txt
//! (empty)
//! =====
//! hill 
//! 
//! the cat walked over the hill
//! 
//! I remember when I was six years old on a hill in yellowstone and almost rolled face-first into a pile of bison dung
//! ```
//! 
//! You can add multiple ` .. `s to create newlines
//! 
//! ## To create a cloze card
//! - ` ...` can be added after term to designate the word that should be clozed. After which, any content after the ` ... ` functions as "extra" content. Please note that there can only be one cloze term per sentence/higlight as of now.
//! 
//! For example, the following note and higlight pair
//! 
//! ```txt
//! the cat walked over the hill
//! ```
//! 
//! ```txt
//! walked ... I remember when I first began walking: my mama balked her eyes out (no I didn't remember)
//! ```
//! 
//! Would create the following cloze card
//! 
//! ```txt
//! the cat {{c1::walked}} over the hill
//! =====
//! I remember when I first began walking: my mama balked her eyes out (no I didn't remember)
//! ```

use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
};

use chrono::prelude::*;
use chrono::serde::ts_seconds;
use clap::{arg, Arg, Command};
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
        #[serde(with = "ts_seconds")]
        date: DateTime<Utc>,
        cards: Vec<Card>,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
enum Card {
    Cloze { front: String, back: String },
    Basic { front: String, back: String },
}

enum OutputFileFormat {
    Yaml,
    Json,
    Markdown,
}

impl OutputFileFormat {
    pub fn get_extension(&self) -> &str {
        match self {
            OutputFileFormat::Yaml => "yml",
            OutputFileFormat::Json => "json",
            OutputFileFormat::Markdown => "md",
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
    let config = Config::new(OutputFileFormat::Markdown);

    // create clap app
    let matches = Command::new("anki-kindle-import")
        .version("0.1.0")
        .author("Andy Li <SpicyRicecaker@gmail.com>")
        .about("Turns kindle clippings into structure easily parsible by Anki")
        .arg(arg!(-v --validate "check the output file to make sure there is one highlight per one note, then compiles it"))
        .arg(Arg::new("start-date")
                .short('d')
                .long("start-date")
                .takes_value(true)
                .help("only include clippings from the start date, inclusive"))
        .arg(Arg::new("clipping-path")
                .short('p')
                .long("clipping-path")
                .takes_value(true)
                .help("the path to kindle clippings. By default points to where Calibre exports clippings. (check README.md)"))
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
                    // at this point we can either split by ` ... ` or ` .. `.
                    // if it's cloze
                    if line.contains(" ... ") {
                        let mut split = line.split(" ... ");
                        // the term in question will be the first split
                        let term = split.next().context("unable to find first term in cloze")?;

                        // attempt to find the term in the previous term, which should be a highlight
                        let clozed_content = if let Clipping::Highlight { sentence, .. } =
                            entries.last().context("no previous highlight for cloze")?
                        {
                            // cloze every match of the term
                            sentence.replace(term, &format!("{{c1::{term}}}"))
                        } else {
                            return Err(Error::msg("Term before cloze entry was not a higlight"));
                        };

                        terms.push(Card::Cloze {
                            front: clozed_content,
                            back: if let Some(extra) = split.next() {
                                extra.to_string()
                            } else {
                                String::new()
                            },
                        });
                    } else if line.contains(" .. ") {
                        let back: Vec<String> = line.split(" .. ").map(|s| s.to_string()).collect();

                        match back.len().cmp(&2) {
                            Ordering::Less => return Err(Error::msg(
                                "no description provided for basic term when using `..` operator",
                            )),
                            Ordering::Equal | Ordering::Greater => {}
                        }

                        terms.push(Card::Basic {
                            front: String::new(),
                            back: back.join("\n"),
                        });
                    } else {
                        terms.push(Card::Basic {
                            front: String::new(),
                            back: line.to_string().to_lowercase(),
                        });
                    }
                }
                entries.push(Clipping::Note {
                    book,
                    author,
                    date,
                    cards: terms,
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
            OutputFileFormat::Markdown => {
                // experimental markdown export
                let mut out_string = String::new();

                // separate entries into
                for entry in entries {
                    match entry {
                        // if it's a highlight, don't even add a bullet, just insert the sentence
                        Clipping::Highlight { sentence, .. } => {
                            out_string.push_str(&format!("========\n{sentence}\n========\n"));
                        }
                        // otherwise, for notes,
                        Clipping::Note { cards, .. } => {
                            for card in cards {
                                match card {
                                    Card::Cloze { front, back } => {
                                        out_string.push_str(&format!(
                                            "----\n{front}\n|-\n{back}\n----\n"
                                        ));
                                    }
                                    Card::Basic { front, back } => {
                                        out_string.push_str(&format!(
                                            "----\n{front}\n|-\n{back}\n----\n"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                out_string
            }
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
            serde_yaml::from_str(&fs::read_to_string(&config.output_file_name).unwrap()).unwrap()
        }
        OutputFileFormat::Json => {
            serde_json::from_str(&fs::read_to_string(&config.output_file_name).unwrap()).unwrap()
        }
        OutputFileFormat::Markdown => {
            // construct array of cards
            let mut cards: Vec<Card> = Vec::new();
            let string = fs::read_to_string(&config.output_file_name).unwrap();

            let mut lines = string.lines();

            let mut sentence = String::new();
            // get next line
            while let Some(line) = lines.next() {
                match line {
                    // match the line to either "========" to signal a sentence, or
                    "========" => {
                        sentence.clear();
                        let mut buffer: Vec<&str> = Vec::new();
                        // consume until next "========"
                        for line in lines.by_ref() {
                            if line != "========" {
                                buffer.push(line);
                            } else {
                                break;
                            }
                        }
                        sentence = buffer.join("\n");
                    }
                    // "----" to signal a card built off that sentence
                    "----" => {
                        let mut buffer: Vec<&str> = Vec::new();
                        // consume until next "========"
                        for line in lines.by_ref() {
                            if line != "----" {
                                buffer.push(line);
                            } else {
                                break;
                            }
                        }
                        let total_content = buffer.join("\n");
                        let mut split = total_content.split("|-");
                        let front = split
                            .next()
                            .context("error splitting front")?
                            .to_string()
                            .trim()
                            .to_string();

                        let back = split
                            .next()
                            .context("error splitting back")?
                            .to_string()
                            .trim()
                            .to_string();

                        // first check for the presence of any cloze beginnings
                        if total_content.contains("{{c1::") {
                            // insert it as a cloze, without the sentence
                            cards.push(Card::Cloze { front, back });
                        } else {
                            // separate the first line of back (the word) from the rest of the content
                            let mut lines = back.lines();
                            let term = lines.next().context("no term provided")?.trim();
                            let rest: String = lines.collect::<String>().trim().to_string();

                            if rest.is_empty() {
                                cards.push(Card::Basic {
                                    front,
                                    back: format!("{}<br><br>{}", term, sentence),
                                });
                            } else {
                                cards.push(Card::Basic {
                                    front,
                                    back: format!("{}<br><br>{}<br><br>{}", term, sentence, rest),
                                });
                            }
                        }
                    }
                    _ => bail!("invalid card sequence detected"),
                }
            }

            fs::write("output.json", serde_json::to_string(&cards).unwrap()).with_context(|| {
                "Unable to write to final output file from cards .md to `out.json` for some reason."
            })?;

            std::process::exit(0);
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

    fs::write("output.json", serde_json::to_string(&vec).unwrap())
        .with_context(|| "Unable to write to final output file `out.json` for some reason.")?;

    Ok(())
}
