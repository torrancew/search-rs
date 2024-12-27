use std::{
    env::args,
    fmt::{self, Display},
};

use search::prelude::*;
use serde::{Deserialize, Serialize};

fn parse_admission_year(data: impl AsRef<str>) -> impl search::xapian::ToValue {
    data.as_ref()[0..4].parse::<u16>().unwrap()
}

#[derive(Deserialize, Schema, Serialize)]
#[search(lang = "english", index, data_fn = Self::to_json)]
pub struct StateInfo {
    #[search(index, prefix = "XS")]
    name: String,
    capital: String,
    #[search(facet_fn = parse_admission_year)]
    admitted: String,
    order: u8,
    #[search(facet)]
    population: u32,
    latitude: String,
    longitude: String,
    #[search(index, prefix = "XM")]
    motto: String,
    midlat: f64,
    midlon: f64,
}

impl StateInfo {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl Display for StateInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}({}): {}",
            self.name, self.admitted, self.motto
        ))
    }
}

fn main() -> anyhow::Result<()> {
    let query = args().skip(1).collect::<Vec<_>>().join(" ");
    let mut indexer = Indexer::<StateInfo>::inmemory()?;

    let states: Vec<StateInfo> =
        csv::Reader::from_reader(&include_bytes!("../../../xapian-rs/tests/data/states.csv")[..])
            .deserialize()
            .collect::<Result<Vec<_>, _>>()
            .expect("Malformed CSV data");

    for s in states {
        indexer.index(s);
    }

    let search = indexer.search(query, 1, None)?;
    for (idx, m) in search.results(0).matches().enumerate() {
        println!(
            "{idx}: {}",
            serde_json::from_str::<StateInfo>(&m.document().to_string())?
        );
    }

    Ok(())
}
