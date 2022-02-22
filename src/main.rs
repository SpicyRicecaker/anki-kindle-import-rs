use std::fs;

use chrono::prelude::*;
use chrono::serde::ts_seconds;
use regex::Regex;
use serde::{Deserialize, Serialize};

use chrono::{TimeZone, Utc};

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
struct Term {
    term: String,
    definition: String,
}

fn main() {
    let clippings_path = {
        let mut t = dirs::home_dir().unwrap();
        t.push(r"Calibre Library/Kindle/My Clippings (13)/My Clippings - Kindle.txt");
        t
    };
    let mut entries = Vec::new();
    let clippings_txt = fs::read_to_string(clippings_path).unwrap();

    let re_author_book = Regex::new(r"(?P<book>.+) \((?P<author>.+)\)").unwrap();
    let re_date = Regex::new(
        // r"- Your (?P<highlight_or_note>.+) on page \d+ \| .+ \| Added on .+, (?P<date>.+,.+)",
        r"- Your (?P<highlight_or_note>.+) on .+ (\| .+ )?\| Added on .+, (?P<date>.+,.+)",
    )
    .unwrap();

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
                        term: line.to_string(),
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
    fs::write("out.json", serde_json::to_string(&entries).unwrap()).unwrap();
}
