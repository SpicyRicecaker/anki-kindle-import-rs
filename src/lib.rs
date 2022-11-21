pub mod config;

use std::path::Path;
use std::{cmp::Ordering, fs};

use anyhow::{bail, Context, Error};

use chrono::prelude::*;
use chrono::serde::ts_seconds;

use log::trace;
use regex::Regex;

use config::Config;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Clipping {
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
pub enum Card {
    Cloze { front: String, back: String },
    Basic { front: String, back: String },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Output {
    cards: Vec<Card>,
    #[serde(with = "ts_seconds")]
    begin_date: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    end_date: DateTime<Utc>,
}

pub fn parse_from_anki(
    clippings_txt: String,
    date_after: Option<DateTime<Utc>>,
) -> Result<Vec<Clipping>, Error> {
    // store all entries
    let mut entries = Vec::new();

    let re_author_book = Regex::new(r"(?P<book>.+) \((?P<author>.+)\)").unwrap();
    let re_date = Regex::new(
        r"- Your (?P<highlight_or_note>.+) on .+ (\| .+ )?\| Added on .+, (?P<date>.+,.+)",
    )?;

    let mut iter = clippings_txt.lines();
    while let Some(line_1) = iter.next() {
        // trace!("{}", line_1);
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

        if let Some(date_after) = date_after {
            if date <= date_after {
                // skip until next ====
                for line in iter.by_ref() {
                    if line.contains("==========") {
                        break;
                    }
                }
                continue;
            }
        }

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
                    if line.contains("...") {
                        let mut split = line.split("...");
                        // the term in question will be the first split
                        let term = split
                            .next()
                            .context("unable to find first term in cloze")?
                            .trim();

                        // attempt to find the term in the previous term, which should be a highlight
                        let clozed_content = if let Clipping::Highlight { sentence, .. } =
                            entries.last().context("no previous highlight for cloze")?
                        {
                            trace!("replacing `{}` in `{}`", term, sentence);
                            let re_term = Regex::new(&format!("(?i)(?P<term>{term})"))?;
                            if re_term.is_match(sentence) {
                                re_term.replace_all(sentence, "{{c1::$term}}").to_string()
                            } else {
                                panic!("no match for {term} in sentence {sentence}")
                            }
                        } else {
                            return Err(Error::msg("Term before cloze entry was not a highlight"));
                        };

                        terms.push(Card::Cloze {
                            // TODO we add two newlines to cloze content because
                            // we also want to be able to manually add word definitions to
                            // the front
                            front: format!("\n\n{clozed_content}"),
                            back: if let Some(extra) = split.next() {
                                // TODO we add two newlines to back because the
                                // reading will be on the back.
                                format!("\n\n{}", extra.trim())
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
    // if let Some(date_inclusive_after) = date_inclusive_after {
    //     entries = entries
    //         .into_iter()
    //         .filter(|c| match c {
    //             Clipping::Highlight { date, .. } => date >= &date_inclusive_after,
    //             Clipping::Note { date, .. } => date >= &date_inclusive_after,
    //         })
    //         .collect();
    // }

    Ok(entries)
}

pub fn run(config: Config) -> Result<(), Error> {
    match config {
        Config::Regular {
            clippings_path,
            output_file_name,
            date_after,
        } => {
            let clippings_txt = fs::read_to_string(clippings_path)
                .with_context(|| "unable to read clippings path")?;

            let entries = parse_from_anki(clippings_txt, date_after)?;

            let out = {
                // experimental markdown export
                let mut out_string = String::new();

                // separate entries into
                for entry in &entries {
                    match entry {
                        // if it's a highlight, don't even add a bullet, just insert the sentence
                        Clipping::Highlight { sentence, .. } => {
                            out_string.push_str(&format!("========\n{sentence}\n========\n"));
                        }
                        // otherwise, for nojes,
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
            };

            write(out, output_file_name)?;

            write(
                serde_json::to_string(&entries)?,
                "out/output-metadata.json".to_string(),
            )?;
        }
        Config::Validate { output_file_name } => {
            validate(output_file_name)?;
        }
    }

    Ok(())
}

pub fn write(out: String, output_file_name: String) -> Result<(), Error> {
    // check if file already exists
    let out_path = Path::new(&output_file_name);
    if out_path.exists() {
        // copy file to `out.json (old)`
        let copy = "out/output-copy.md";
        fs::copy(out_path, copy).with_context(|| {
            format!(
                "unable to copy from {:#?} to {:#?} for some reason",
                out_path, copy
            )
        })?;
        println!("overwrote old {:?} (backed up to `{:?}`)", out_path, copy);
    }
    // copy to something
    fs::write(&output_file_name, out)
        .with_context(|| "Unable to write to final output file `out.json/yml` for some reason.")?;

    Ok(())
}

/// Validates if notes have one highlight and one (or more) terms. This doesn't
/// work for cloze types, so it's kinda useless rn
fn validate(output_file_name: String) -> Result<(), Error> {
    // parse the file in to JSON
    // construct array of cards
    let mut cards: Vec<Card> = Vec::new();
    let string = fs::read_to_string(&output_file_name).unwrap();

    let mut lines = string.lines().enumerate();

    let mut sentence = String::new();
    // get next line
    while let Some((number, line)) = lines.next() {
        match line {
            // match the line to either "========" to signal a sentence, or
            "========" => {
                sentence.clear();
                let mut buffer: Vec<&str> = Vec::new();
                // consume until next "========"
                for (_, line) in lines.by_ref() {
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
                for (_, line) in lines.by_ref() {
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
                    .context(format!("The content in the card does not have a `front` side! Card with content `{}`", total_content))?
                    .to_string()
                    .trim()
                    .to_string();

                let back = split
                    .next()
                    .context(format!("The content in the card does not have a `back` side! Card with content `{}`", total_content))?
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
                    let term = lines
                        .next()
                        .with_context(|| format!("no term provided for {}", string))?
                        .trim();
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
            _ => {
                bail!(
                    "invalid card sequence detected `{}` at line number {}",
                    line,
                    number
                )
            }
        }
    }

    let metadata: Vec<Clipping> =
        serde_json::from_str(&fs::read_to_string("out/output-metadata.json")?)?;

    let output = Output {
        cards,
        begin_date: match metadata
            .first()
            .context("no first element in output-metadata.json")?
        {
            Clipping::Highlight { date, .. } => *date,
            Clipping::Note { date, .. } => *date,
        },
        end_date: match metadata
            .last()
            .context("no last element in output-metadata.json")?
        {
            Clipping::Highlight { date, .. } => *date,
            Clipping::Note { date, .. } => *date,
        },
    };

    fs::write("out/output.json", serde_json::to_string(&output).unwrap()).with_context(|| {
        "Unable to write to final output file from cards .md to `out.json` for some reason."
    })?;

    Ok(())
}
