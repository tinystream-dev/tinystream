// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use sqlx::SqlitePool;

pub const DEFAULT_FILE: &str = "{show} - S{season:00}E{episode:00} - {title}";
pub const DEFAULT_FOLDER: &str = "Season {season:00}";

#[derive(Debug, Clone, Default)]
pub struct Values {
    pub show: String,
    pub year: Option<i64>,
    pub season: u32,
    pub episode: u32,
    pub title: Option<String>,
    pub group: Option<String>,

    pub quality: Option<String>,

    pub codec: Option<String>,

    pub original: String,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "NamingStyle")]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub file: String,
    pub folder: String,

    pub agreement: Option<f32>,
    pub samples: usize,
}

static SXXEYY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(s)(\d{1,3})([ ._]?)(e)(\d{1,4})(?:-?e\d{1,4})?").unwrap());
static LEADING_GROUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[([^\]]+)\]").unwrap());
static QUALITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(2160|1080|720|576|480)p\b").unwrap());
pub static CODEC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(x\.?265|h\.?265|hevc|x\.?264|h\.?264|avc|av1)\b").unwrap());
static CRC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\[[0-9A-Fa-f]{8}\]").unwrap());
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(\w+)(?::(0+))?\}").unwrap());
static SEASON_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(season|series|s)([ ._]?)(\d{1,3})$").unwrap());

fn pad(width: usize) -> String {
    if width <= 1 { String::new() } else { format!(":{}", "0".repeat(width)) }
}

fn replace_ci(hay: &str, needle: &str, with: &str) -> Option<String> {
    if needle.trim().is_empty() {
        return None;
    }

    let lower = hay.to_lowercase();
    let at = lower.find(&needle.to_lowercase())?;

    if !hay.is_char_boundary(at) || !hay.is_char_boundary(at + needle.len()) {
        return None;
    }

    Some(format!("{}{}{}", &hay[..at], with, &hay[at + needle.len()..]))
}

pub fn template_of(stem: &str, show_titles: &[String], year: Option<i64>, title: Option<&str>) -> Option<String> {
    let caps = SXXEYY.captures(stem)?;

    let token = format!(
        "{}{{season{}}}{}{}{{episode{}}}",
        &caps[1],
        pad(caps[2].len()),
        &caps[3],
        &caps[4],
        pad(caps[5].len())
    );

    let m = caps.get(0).unwrap();
    let mut t = format!("{}{}{}", &stem[..m.start()], token, &stem[m.end()..]);
    t = CRC.replace_all(&t, "").to_string();

    if let Some(c) = LEADING_GROUP.captures(&t) {
        let g = c[1].to_string();

        if !QUALITY.is_match(&g) {
            t = format!("[{{group}}]{}", &t[c.get(0).unwrap().end()..]);
        }
    }

    if let Some(title) = title.filter(|x| x.len() >= 2)
        && let Some(r) = replace_ci(&t, title, "{title}")
    {
        t = r;
    }

    for show in show_titles {
        let variants = [
            show.clone(),
            show.replace(' ', "."),
            show.replace(' ', "_"),
            show.replace(':', ""),
            show.replace(':', " -"),
        ];

        if let Some(r) = variants.iter().find_map(|v| replace_ci(&t, v, "{show}")) {
            t = r;
            break;
        }
    }

    if let Some(y) = year {
        t = t.replacen(&format!("({y})"), "({year})", 1);
    }

    t = QUALITY.replace(&t, "{quality}").to_string();
    t = CODEC.replace(&t, "{codec}").to_string();
    Some(t.trim().to_string())
}

pub fn folder_template_of(name: &str) -> Option<String> {
    if name.eq_ignore_ascii_case("specials") {
        return None;
    }

    let c = SEASON_WORD.captures(name.trim())?;
    Some(format!("{}{}{{season{}}}", &c[1], &c[2], pad(c[3].len())))
}

fn majority(templates: impl Iterator<Item = String>) -> Option<(String, usize, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0;

    for t in templates {
        *counts.entry(t).or_default() += 1;
        total += 1;
    }

    let (best, n) = counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))?;
    Some((best, n, total))
}

struct FileRow {
    item_path: String,
    path: String,
    title: Option<String>,
    item_title: String,
    folder_title: String,
    year: Option<i64>,
}

async fn rows(db: &SqlitePool, filter: &str, value: &str) -> anyhow::Result<Vec<FileRow>> {
    let sql = format!(
        "SELECT i.path, m.path, m.title, i.title, i.folder_title, COALESCE(i.folder_year, i.year)
         FROM media m JOIN items i ON i.id = m.item_id WHERE i.kind = 'show' AND m.season IS NOT NULL AND {filter} = ?"
    );

    let rows: Vec<(String, String, Option<String>, String, String, Option<i64>)> =
        sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(value).fetch_all(db).await?;

    Ok(rows
        .into_iter()
        .map(|(item_path, path, title, item_title, folder_title, year)| FileRow {
            item_path,
            path,
            title,
            item_title,
            folder_title,
            year,
        })
        .collect())
}

fn style_from(rows: &[FileRow]) -> Option<Style> {
    let files = rows.iter().filter_map(|r| {
        let stem = Path::new(&r.path).file_stem()?.to_string_lossy().to_string();
        template_of(&stem, &[r.folder_title.clone(), r.item_title.clone()], r.year, r.title.as_deref())
    });

    let (file, n, total) = majority(files)?;

    let folders = rows.iter().filter_map(|r| {
        let season_dir = Path::new(&r.path).parent()?.file_name()?.to_string_lossy().to_string();
        folder_template_of(&season_dir)
    });

    let folder = majority(folders).map(|(f, _, _)| f).unwrap_or_else(|| DEFAULT_FOLDER.to_string());
    Some(Style { file, folder, agreement: Some(n as f32 / total as f32), samples: total })
}

pub async fn show_style(db: &SqlitePool, show_path: &str) -> anyhow::Result<Option<Style>> {
    Ok(style_from(&rows(db, "i.path", show_path).await?))
}

pub async fn library_style(db: &SqlitePool, library: &str) -> anyhow::Result<Style> {
    let all = rows(db, "i.library", library).await?;
    let mut by_show: HashMap<&str, Vec<&FileRow>> = HashMap::new();

    for r in &all {
        by_show.entry(r.item_path.as_str()).or_default().push(r);
    }

    let mut files = Vec::new();
    let mut folders = Vec::new();

    for rows in by_show.values() {
        let owned: Vec<FileRow> = rows
            .iter()
            .map(|r| FileRow {
                item_path: r.item_path.clone(),
                path: r.path.clone(),
                title: r.title.clone(),
                item_title: r.item_title.clone(),
                folder_title: r.folder_title.clone(),
                year: r.year,
            })
            .collect();

        if let Some(s) = style_from(&owned) {
            files.push(s.file);
            folders.push(s.folder);
        }
    }

    let shows = files.len();

    Ok(match (majority(files.into_iter()), majority(folders.into_iter())) {
        (Some((file, n, total)), folder) => Style {
            file,
            folder: folder.map(|f| f.0).unwrap_or_else(|| DEFAULT_FOLDER.into()),
            agreement: Some(n as f32 / total as f32),
            samples: shows,
        },
        _ => Style { file: DEFAULT_FILE.into(), folder: DEFAULT_FOLDER.into(), agreement: None, samples: 0 },
    })
}

pub async fn library_uses_years(db: &SqlitePool, library: &str) -> anyhow::Result<bool> {
    let (with, total): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(folder_year IS NOT NULL), 0), COUNT(*) FROM items WHERE library = ? AND kind = 'show'",
    )
    .bind(library)
    .fetch_one(db)
    .await?;

    Ok(total > 0 && with * 2 > total)
}

pub fn render(template: &str, v: &Values) -> String {
    let out = TOKEN.replace_all(template, |c: &regex::Captures| {
        let width = c.get(2).map(|m| m.as_str().len()).unwrap_or(0);
        let num = |n: u32| format!("{n:0width$}");

        match &c[1] {
            "show" => v.show.clone(),
            "year" => v.year.map(|y| y.to_string()).unwrap_or_default(),
            "season" => num(v.season),
            "episode" => num(v.episode),
            "title" => v.title.clone().unwrap_or_default(),
            "group" => v.group.clone().unwrap_or_default(),
            "quality" => v.quality.clone().unwrap_or_default(),
            "codec" => v.codec.clone().unwrap_or_default(),
            "original" => v.original.clone(),
            _ => c[0].to_string(),
        }
    });

    tidy(&sanitize(&out))
}

pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        match c {
            ':' => out.push_str(if chars.get(i + 1) == Some(&' ') { " -" } else { "-" }),
            '/' | '\\' => out.push('-'),
            '?' | '*' | '"' | '<' | '>' | '|' | '\0' => {},
            _ => out.push(c),
        }
    }

    out
}

fn tidy(s: &str) -> String {
    let mut s = s.to_string();

    loop {
        let before = s.clone();

        for (from, to) in [
            ("[]", ""),
            ("()", ""),
            ("[ ]", ""),
            ("  ", " "),
            (" - - ", " - "),
            (" - [", " ["),
            (" - (", " ("),
            ("..", "."),
            (" .", "."),
            ("-.", "."),
        ] {
            s = s.replace(from, to);
        }

        if s == before {
            break;
        }
    }

    s.trim_matches(|c: char| c == ' ' || c == '-' || c == '.' || c == '_').trim_end_matches(" -").to_string()
}

pub fn validate(template: &str) -> Result<(), String> {
    let sample = render(
        template,
        &Values { show: "Show".into(), season: 3, episode: 7, original: "Show S03E07".into(), ..Values::default() },
    );

    match crate::library::parse::episode_number(&sample) {
        Some(e) if e.season == 3 && e.episode == 7 => Ok(()),
        _ => Err(format!(
            "“{template}” gives names like “{sample}”, which don't carry an episode number like S03E07; include S{{season:00}}E{{episode:00}}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_templates() {
        let t = template_of(
            "[SubsPlease] Chainsaw Man - S01E05 - Gun Devil [1080p] [ABCDEF12]",
            &["Chainsaw Man".into()],
            None,
            Some("Gun Devil"),
        )
        .unwrap();

        assert_eq!(t, "[{group}] {show} - S{season:00}E{episode:00} - {title} [{quality}]");
        let t = template_of("Chainsaw.Man.S01E05.1080p.WEB.x264", &["Chainsaw Man".into()], None, None).unwrap();
        assert_eq!(t, "{show}.S{season:00}E{episode:00}.{quality}.WEB.{codec}");
        assert_eq!(folder_template_of("Season 01").as_deref(), Some("Season {season:00}"));
        assert_eq!(folder_template_of("S2").as_deref(), Some("S{season}"));
    }

    #[test]
    fn renders_and_drops_unknowns() {
        let v = Values {
            show: "Re:Zero".into(),
            season: 4,
            episode: 19,
            group: Some("SubsPlease".into()),
            quality: Some("1080p".into()),
            ..Values::default()
        };

        assert_eq!(
            render("[{group}] {show} - S{season:00}E{episode:00} - {title} [{quality}]", &v),
            "[SubsPlease] Re-Zero - S04E19 [1080p]"
        );

        assert_eq!(render(DEFAULT_FILE, &Values { group: None, ..v.clone() }), "Re-Zero - S04E19");
        assert_eq!(render("Season {season:00}", &v), "Season 04");
    }

    #[test]
    fn validation() {
        assert!(validate(DEFAULT_FILE).is_ok());
        assert!(validate("{show} - {episode}").is_err());
    }

    #[test]
    fn majority_wins() {
        let (t, n, total) = majority(["a", "b", "a"].into_iter().map(String::from)).unwrap();
        assert_eq!((t.as_str(), n, total), ("a", 2, 3));
    }
}
