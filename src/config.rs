use std::path::PathBuf;

use anyhow::Error;
use chrono::prelude::*;
use clap::{arg, Arg, Command};
use log::info;

pub enum Config {
    Regular {
        clippings_path: PathBuf,
        output_file_name: String,
        date_inclusive_after: Option<DateTime<Utc>>,
    },
    Validate {
        output_file_name: String,
    },
}

impl Config {
    pub fn new() -> Result<Config, Error> {
        let output_file_name = String::from("output.md");

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
            Ok(Config::Validate { output_file_name })
        } else {
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

            Ok(Config::Regular {
                output_file_name,
                clippings_path,
                date_inclusive_after,
            })
        }
    }
}

fn date_from_str(date_str: &str) -> Result<DateTime<Utc>, Error> {
    let naive_time = NaiveTime::from_hms(0, 0, 0);
    let naive_date = NaiveDate::parse_from_str(date_str, "%m-%d-%Y")?;
    let naive_date_time = NaiveDateTime::new(naive_date, naive_time);
    Ok(Local.from_local_datetime(&naive_date_time).unwrap().into())
}
