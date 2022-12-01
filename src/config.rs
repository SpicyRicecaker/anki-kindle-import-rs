use std::{fs, path::PathBuf};

use anyhow::{Context, Error};
use chrono::prelude::*;
use chrono::serde::ts_seconds;
use clap::{Arg, ArgAction, Command};
use log::info;
use serde::{Deserialize, Serialize};

pub enum Config {
    Regular {
        clippings_path: PathBuf,
        output_file_name: String,
        date_after: Option<DateTime<Utc>>,
    },
    Validate {
        output_file_name: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastDate {
    #[serde(with = "ts_seconds")]
    date: DateTime<Utc>,
}

impl Config {
    pub fn new() -> Result<Config, Error> {
        let output_file_name = String::from("out/output.md");
        // ensure dir
        std::fs::create_dir_all("out")?;

        // create clap app
        let matches = Command::new("anki-kindle-import")
        .version("0.1.0")
        .author("Andy Li <SpicyRicecaker@gmail.com>")
        .about("Turns kindle clippings into structure easily parsible by Anki")
        .arg(Arg::new("validate")
                .short('v')
                .long("validate")
                .action(ArgAction::Count)
                .help("check the output file to make sure there is one highlight per one note, then compiles it")
        )
        .arg(Arg::new("start-date")
                .short('d')
                .long("start-date")
                .action(ArgAction::Set)
                // .takes_value(true)
                .help("only include clippings from the start date, inclusive"))
        .arg(Arg::new("clipping-path")
                .short('p')
                .long("clipping-path")
                .action(ArgAction::Set)
                // .takes_value(true)
                .help("the path to kindle clippings. By default points to where Calibre exports clippings. (check README.md)"))
        .get_matches();

        // check if we should validate, and continue on with the rest of the program
        if matches.get_count("validate") > 0 {
            Ok(Config::Validate { output_file_name })
        } else {
            // get optional argument if needed
            let date_after = if let Some(date_string) = matches.get_one::<String>("start-date") {
                Some(date_from_str(date_string)?)
            // last-date.json is written by Anki, after last feed
            // we probably need testing for this, because this is getting too complex
            } else if let Ok(file) = fs::read_to_string("out/last-date.json") {
                let last_date: LastDate = serde_json::from_str(&file)?;
                Some(last_date.date)
            } else {
                None
            };

            // get clipping path & reading clipping
            let clippings_path = if let Some(p) = matches.get_one::<String>("clipping-path") {
                PathBuf::from(p)
            } else {
                // hardcoded scan for kindle directory
                // this might be broken...I think `fetch annotations` from
                // calibre refreshes this file or something, it may not be
                // updated right away
                let opt_1 = match std::env::consts::OS {
                    "macos" => PathBuf::from("/Volumes/Kindle/documents/My Clippings.txt"),
                    _ => {
                        panic!("not implemented")
                    }
                };
                let mut opt_2 = dirs::home_dir().unwrap();
                opt_2.push("/Calibre Library/Kindle/My Clippings (13)/My Clippings - Kindle.txt");


                [opt_1, opt_2].into_iter().find(|p| p.exists()).unwrap()
            };

            Ok(Config::Regular {
                output_file_name,
                clippings_path,
                date_after,
            })
        }
    }
}

fn date_from_str(date_str: &str) -> Result<DateTime<Utc>, Error> {
    let naive_time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let naive_date = NaiveDate::parse_from_str(date_str, "%m-%d-%Y")
        .with_context(|| "error parsing the start date. Valid format is month-day-year")?;
    let naive_date_time = NaiveDateTime::new(naive_date, naive_time);
    info!("using clippings past start date: {}", naive_date_time);
    Ok(Local.from_local_datetime(&naive_date_time).unwrap().into())
}
