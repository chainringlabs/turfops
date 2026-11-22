//! The `score-estimate` command: fetches the public data it needs into a cache directory
//! (anything already there is reused), scores the estimate, and writes the data file.
//!
//! `cargo run --features scoring -- score-estimate --cache <dir> --out <file>`

use super::inputs::{parse_daily01, parse_grid, StationSoil};
use super::score::{score, Station};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DAILY01: &str = "https://www.ncei.noaa.gov/pub/data/uscrn/products/daily01";
const GRID: &str = "https://grid2.rcc-acis.org/GridData";
/// PRISM daily, as RCC-ACIS numbers its grids.
const PRISM_GRID: &str = "21";
const STATION_PAUSE: Duration = Duration::from_secs(1);
const GRID_PAUSE: Duration = Duration::from_secs(2);
/// States of the contiguous United States (and DC), as station file names begin.
const CONTIGUOUS: &str = "AL AZ AR CA CO CT DE DC FL GA ID IL IN IA KS KY LA ME MD MA MI MN MS \
    MO MT NE NV NH NJ NM NY NC ND OH OK OR PA RI SC SD TN TX UT VT VA WA WV WI WY";

struct Options {
    cache: PathBuf,
    out: PathBuf,
    first_year: i32,
    last_year: i32,
}

fn options(args: &[String]) -> Result<Options> {
    let value = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
    };
    let year = |flag: &str, default: i32| -> Result<i32> {
        value(flag).map_or(Ok(default), |text| {
            text.parse()
                .with_context(|| format!("{flag} is not a year"))
        })
    };
    Ok(Options {
        cache: value("--cache")
            .context("--cache <dir> is required")?
            .into(),
        out: value("--out").context("--out <file> is required")?.into(),
        first_year: year("--first-year", 2012)?,
        last_year: year("--last-year", 2025)?,
    })
}

/// Runs the command. `args` are the process arguments after `score-estimate`.
// @spec WX-EST-SCORE-001, WX-EST-SCORE-004
pub async fn run(args: &[String]) -> Result<()> {
    let options = options(args)?;
    let client = reqwest::Client::builder()
        .user_agent(concat!("TurfOps/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .build()?;
    let names = station_names(&client, &options).await?;
    println!("{} stations in the contiguous United States", names.len());
    let mut stations = Vec::new();
    for name in &names {
        let soil = station_soil(&client, &options, name).await?;
        let air = grid_air(&client, &options, name, &soil).await?;
        stations.push(Station {
            name: name.clone(),
            soil,
            air,
        });
    }
    let today = chrono::Utc::now().date_naive();
    let file = score(&stations, options.first_year, options.last_year, today);
    std::fs::write(&options.out, serde_json::to_string_pretty(&file)? + "\n")?;
    println!(
        "{} crossings scored → {}",
        file.crossings.len(),
        options.out.display()
    );
    Ok(())
}

fn daily_name(year: i32, station: &str) -> String {
    format!("CRND0103-{year}-{station}.txt")
}

/// Station names (as in the file names) from the last year's cached files, or from the
/// network's directory listing when the cache has none.
async fn station_names(client: &reqwest::Client, options: &Options) -> Result<Vec<String>> {
    let year = options.last_year;
    let dir = options.cache.join("daily01").join(year.to_string());
    let prefix = format!("CRND0103-{year}-");
    let mut names: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    if names.is_empty() {
        let listing = client.get(format!("{DAILY01}/{year}/")).send().await?;
        let listing = listing.error_for_status()?.text().await?;
        names = listing
            .split('"')
            .filter(|part| part.starts_with(&prefix) && part.ends_with(".txt"))
            .map(str::to_string)
            .collect();
    }
    let mut names: Vec<String> = names
        .iter()
        .filter_map(|file| file.strip_prefix(&prefix)?.strip_suffix(".txt"))
        .filter(|name| {
            CONTIGUOUS
                .split_whitespace()
                .any(|state| name.starts_with(&format!("{state}_")))
        })
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        bail!("no station files found for {year}");
    }
    Ok(names)
}

/// The text at `path`, fetched with `fetch` and saved there first when it is not cached.
/// A missing station-year (404) is cached as an empty file.
async fn cached(path: &Path, fetch: reqwest::RequestBuilder, pause: Duration) -> Result<String> {
    if let Ok(text) = std::fs::read_to_string(path) {
        return Ok(text);
    }
    tokio::time::sleep(pause).await;
    let response = fetch.send().await?;
    let text = if response.status() == reqwest::StatusCode::NOT_FOUND {
        String::new()
    } else {
        response.error_for_status()?.text().await?
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &text)?;
    Ok(text)
}

async fn station_soil(
    client: &reqwest::Client,
    options: &Options,
    name: &str,
) -> Result<StationSoil> {
    let mut soil = StationSoil::default();
    for year in options.first_year..=options.last_year {
        let file = daily_name(year, name);
        let path = options
            .cache
            .join("daily01")
            .join(year.to_string())
            .join(&file);
        let fetch = client.get(format!("{DAILY01}/{year}/{file}"));
        parse_daily01(&cached(&path, fetch, STATION_PAUSE).await?, &mut soil);
    }
    Ok(soil)
}

/// The grid's daily air at the station's point, from two months before the first year so
/// the 30-day trailing mean is full on its first day.
async fn grid_air(
    client: &reqwest::Client,
    options: &Options,
    name: &str,
    soil: &StationSoil,
) -> Result<super::super::model::DailyAir> {
    let path = options.cache.join("acis21").join(format!("{name}.json"));
    let body = serde_json::json!({
        "loc": format!("{},{}", soil.longitude, soil.latitude),
        "sdate": format!("{}-11-01", options.first_year - 1),
        "edate": format!("{}-12-31", options.last_year),
        "grid": PRISM_GRID,
        "elems": [{"name": "maxt"}, {"name": "mint"}],
    });
    let text = cached(&path, client.post(GRID).json(&body), GRID_PAUSE).await?;
    parse_grid(&text).with_context(|| format!("grid response for {name}"))
}
