use std::{
    fs,
    path::{self, Path},
};

use chrono::prelude::*;
use chrono::serde::ts_seconds;
use log::info;
use regex::Regex;
use serde::{Deserialize, Serialize};

use anyhow::{bail, Error, Context};
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

fn main() -> Result<(), Error>{
    // initialize logger
    env_logger::init();
    info!("Program started");

    // check if we should validate, and continue on with the rest of the program
    validate()?;

    // get optional argument if needed
    let date_inclusive_after = get_date_from_arg();

    let clippings_path = {
        let mut t = dirs::home_dir().unwrap();
        t.push(r"Calibre Library/Kindle/My Clippings (13)/My Clippings - Kindle.txt");
        t
    };
    let mut entries = Vec::new();
    let clippings_txt = fs::read_to_string(clippings_path).with_context(|| "unable to read clippings path")?;

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
    let out_path = Path::new("out.json");
    if out_path.exists() {
        // copy file to `out.json (old)`
        let copy = Path::new("out-copy.json");
        fs::copy(out_path, copy).with_context(||format!("unable to copy from {:#?} to {:#?} for some reason", out_path, copy))?;
        println!("overwrote old `out.json` (backed up to `out-copy.json`)");
    }
    // copy to something
    fs::write("out.json", serde_json::to_string(&entries).unwrap()).with_context(||"Unable to write to final output file `out.json` for some reason.")?;
    Ok(())
}

fn get_date_from_arg() -> Option<DateTime<Utc>> {
    let mut args = std::env::args();
    args.next();
    match args.next() {
        Some(arg) => {
            info!("Program started with arg {}", arg);
            let naive_time = NaiveTime::from_hms(0, 0, 0);
            let naive_date = NaiveDate::parse_from_str(&arg, "%m-%d-%Y").unwrap();
            let naive_date_time = NaiveDateTime::new(naive_date, naive_time);
            Some(Local.from_local_datetime(&naive_date_time).unwrap().into())
        }
        None => {
            info!("Program started with no args");
            None
        }
    }
}

// #[derive(Error, Debug)]
// enum ParsingError {
// #[error("Invalid header (expected {expected:?}, got {found:?})")]
//     InvalidHeader {
//         expected: String,
//         found: String,
//     },
//     #[error("Missing attribute: {0}")]
//     MissingAttribute(String),
// }

/// Validates if notes have one highlight and one (or more) terms. This doesn't
/// work for cloze types, so it's kinda useless rn
fn validate() -> Result<(), Error> {
    // ignore the first arg, which is always the program path
    let mut args = std::env::args();
    args.next();

    // if there is in fact a "validate" argument
    if let Some(arg) = args.next() {
        if arg == "validate" {
            // parse the file in to JSON
            let vec: Vec<Clipping> =
                serde_json::from_str(&std::fs::read_to_string("out.json").unwrap()).unwrap();

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
            })
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}
