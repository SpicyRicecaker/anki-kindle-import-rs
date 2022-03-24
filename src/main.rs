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

use anki_kindle_import::{config::Config, run};
use env_logger::Env;
use log::info;

use anyhow::Error;

fn main() -> Result<(), Error> {
    // initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("trace")).init();
    info!("Program started");

    // generate config
    run(Config::new()?)?;

    Ok(())
}
