#[macro_use]
extern crate log;

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use clap::Parser;
use regex::Regex;
use std::{env, path::PathBuf};

lazy_static::lazy_static! {
    static ref DIR_PATTERN: Regex = Regex::new(
        r#"(?P<year>\d{4})(\.?)(?P<month>\d{2})(\.?)(?P<day>\d{2})[\-\s](?P<streamer>.+\-)?(?P<title>.+)"#
    ).unwrap();
    static ref FILE_PATTERN: Regex = Regex::new(
        r#"(\d{2}(\.)?\d{2}(\.?)\d{2})[\-\s](?P<hour>\d{2})?(?P<minute>\d{2})?(?P<second>\d{2})?([\-\s]?)(【(.+)】)?(?P<title>.+)\.(?P<ext>[^\.]+)"#
    ).unwrap();
}

#[derive(Debug, Parser)]
struct Opts {
    path: PathBuf,

    #[clap(long = "force", short = 'f')]
    force: bool,
}

fn main() -> Result<()> {
    let opt = Opts::parse();

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug");
    }
    pretty_env_logger::try_init_timed()?;

    let root = opt.path;
    for path in root.read_dir()? {
        let path = path?.path();
        if !path.is_dir() {
            continue;
        }
        let path_s = path.display().to_string();
        if let Some(cap) = DIR_PATTERN.captures(&path_s) {
            let (year, month, day) = (
                cap.name("year").unwrap().as_str().parse()?,
                cap.name("month").unwrap().as_str().parse()?,
                cap.name("day").unwrap().as_str().parse()?,
            );
            let date = NaiveDate::from_ymd(year, month, day);
            run_dir(path, date, opt.force)?;
        } else {
            warn!("文件夹 {} 匹配失败", path.display());
        }
    }

    Ok(())
}

fn run_dir(path: PathBuf, date: NaiveDate, force: bool) -> Result<()> {
    debug!("running {}", path.display());
    let mut count = 0;
    for f in path.read_dir()? {
        let f = f?.path();
        trace!("{} ext = {:?}", f.display(), f.extension());
        match f.extension() {
            Some(ext) if ext == "flv" || ext == "mp4" => {
                let nfo = f.with_extension("nfo");
                if !nfo.exists()
                    || force
                    || nfo.metadata()?.modified()? <= f.metadata()?.modified()?
                {
                    if let Err(e) = generate(f, nfo, date) {
                        error!("{}", e);
                    } else {
                        count += 1;
                    }
                }
            }
            _ => continue,
        }
    }
    if count == 0 {
        debug!("no files found in {}", path.display());
    }

    Ok(())
}

fn generate(media: PathBuf, nfo: PathBuf, date: NaiveDate) -> Result<()> {
    info!("generating {}", nfo.display());
    // get time
    let media_filename = media
        .file_name()
        .context("解析文件名失败")?
        .to_str()
        .context("文件名无法转换为 String")?;
    let cap = FILE_PATTERN
        .captures(media_filename)
        .with_context(|| format!("文件名 `{}` 匹配正则失败", media.display()))?;
    let datetime = NaiveDateTime::new(
        date,
        NaiveTime::from_hms(
            cap.name("hour")
                .map(|m| m.as_str())
                .unwrap_or("20")
                .parse()?,
            cap.name("minute")
                .map(|m| m.as_str())
                .unwrap_or("00")
                .parse()?,
            cap.name("second")
                .map(|m| m.as_str())
                .unwrap_or("00")
                .parse()?,
        ),
    );

    let datetime_s = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
    let date_s = datetime.format("%Y-%m-%d").to_string();
    let title = cap.name("title").unwrap().as_str();
    let year = date.year();

    let content = format!(
        r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?>
    <movie>
        <dateadded>{datetime_s}</dateadded>
        <title>{title}</title>
        <originaltitle>{title}</originaltitle>
        <year>{year}</year>
        <premiered>{date_s}</premiered>
        <releasedate>{date_s}</releasedate>
        <tag>A-SOUL</tag>
        <set>
            <name>A-SOUL</name>
        </set>
    </movie>"#
    );
    info!("nfo {} generated.", nfo.display());

    std::fs::write(nfo, content)?;

    Ok(())
}
