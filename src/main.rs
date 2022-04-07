#[macro_use]
extern crate log;

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use clap::Parser;
use regex::Regex;
use std::{env, path::PathBuf};

lazy_static::lazy_static! {
    static ref DIR_PATTERN: Regex = Regex::new(
        r#"\d{4}(\.?)\d{2}(\.?)\d{2}[\-\s]((?P<streamer>.+)[\-\s])?(?P<title>.+)"#
    ).unwrap();
    static ref FILE_PATTERN: Regex = Regex::new(
        concat!(
            r#"^(?P<year>\d{4})\.?(?P<month>\d{2})\.?(?P<day>\d{2})"#,
            r#"[\-\s]"#,
            r#"((?P<hour>\d{2})(?P<minute>\d{2})(?P<second>\d{2})[\-\s])?"#,
            r#"(【(.+)】)?(?P<title>.+)\.(?P<ext>[^\.]+)$"#
        )
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
        if DIR_PATTERN.is_match(&path_s) {
            run_dir(path, opt.force)?;
        } else {
            warn!("文件夹 {} 匹配失败", path.display());
        }
    }

    Ok(())
}

fn run_dir(path: PathBuf, force: bool) -> Result<()> {
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
                    if let Err(e) = generate(f, nfo) {
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

fn extract_filename(filename: &str) -> Result<(NaiveDateTime, &str)> {
    let cap = FILE_PATTERN
        .captures(filename)
        .with_context(|| format!("文件名 `{}` 匹配正则失败", filename))?;
    let (year, month, day) = (
        cap.name("year").unwrap().as_str().parse()?,
        cap.name("month").unwrap().as_str().parse()?,
        cap.name("day").unwrap().as_str().parse()?,
    );
    let date = NaiveDate::from_ymd(year, month, day);
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
    let title = cap.name("title").unwrap().as_str();

    Ok((datetime, title))
}

fn generate(media: PathBuf, nfo: PathBuf) -> Result<()> {
    info!("generating {}", nfo.display());
    // get time
    let media_filename = media
        .file_name()
        .context("解析文件名失败")?
        .to_str()
        .context("文件名无法转换为 String")?;

    let (datetime, title) = extract_filename(media_filename)?;

    let datetime_s = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
    let date_s = datetime.format("%Y-%m-%d").to_string();
    let year = datetime.year();

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_filename() {
        let (_dt, title) =
            extract_filename("20210818-210116-【3D】看！看点儿啥呢！！！.flv").unwrap();
        assert_eq!(title, "看！看点儿啥呢！！！");

        let (_dr, title) = extract_filename("20210214 今天是情人节呢~聊天杂谈.flv").unwrap();
        assert_eq!(title, "今天是情人节呢~聊天杂谈");
    }

    #[test]
    fn capture_dir() {
        assert!(DIR_PATTERN.is_match("20210116-乃琳 温柔夜谈"));
        assert!(DIR_PATTERN.is_match("20210220 A-SOUL小剧场 第八期 燃烧吧！卡路里！"));
    }
}
