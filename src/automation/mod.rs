// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod engine;
pub mod fsops;
pub mod import;
pub mod matching;
pub mod monitor;
pub mod naming;
pub mod profile;
pub mod release;
pub mod renames;
pub mod series;
pub mod sources;

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use anyhow::{Context, bail};
use matching::{Match, Matcher};
use profile::{Rules, Verdict};
use release::Attributes;
use serde::{Deserialize, Serialize};
use sources::{Release, Sources, Torrent};
use tokio::sync::Notify;

use crate::config::{Config, Seeding};
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

pub struct Automation {
    pub engine: engine::Engine,
    pub sources: Sources,
    wake: Notify,
}

impl Automation {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        Ok(Self { engine: engine::Engine::start(config)?, sources: Sources::new(), wake: Notify::new() })
    }

    pub fn wake(&self) {
        self.wake.notify_one();
    }
}

static LOCAL_OFFSET: OnceLock<time::UtcOffset> = OnceLock::new();

pub fn init_local_offset() {
    let _ = LOCAL_OFFSET.set(time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC));
}

pub fn local_offset() -> time::UtcOffset {
    LOCAL_OFFSET.get().copied().unwrap_or(time::UtcOffset::UTC)
}

pub fn spawn(state: Arc<AppState>) {
    engine::spawn(state.clone());
    monitor::spawn(state.clone());
    renames::spawn(state);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    #[serde(flatten)]
    pub release: Release,
    pub attributes: Attributes,

    pub episodes: Vec<(u32, u32)>,
    pub batch: bool,
    pub verdict: Verdict,
}

pub struct ShowSearch {
    pub matcher: Matcher,
    pub rules: Rules,
    pub sources: Vec<crate::config::Source>,

    pub pinned: bool,
}

pub async fn show_search(state: &AppState, series_id: i64) -> anyhow::Result<ShowSearch> {
    let config = state.config.current();
    let row = series::get(state, series_id).await?;
    let matcher = Matcher::load(&state.db, series_id).await?;
    let profile_name = row.profile.clone().or_else(|| config.library(&row.library).and_then(|l| l.profile.clone()));

    let profile = profile_name
        .as_deref()
        .and_then(|p| config.profile(p))
        .or(config.profiles.first())
        .cloned()
        .unwrap_or_default();

    let rules = Rules::new(&profile, &series::list(&row.groups));
    let pinned = series::list(&row.sources);

    let sources: Vec<_> = if pinned.is_empty() {
        config.sources.iter().filter(|s| s.enabled).cloned().collect()
    } else {
        pinned.iter().filter_map(|n| config.source(n)).cloned().collect()
    };

    Ok(ShowSearch { matcher, rules, sources, pinned: !pinned.is_empty() })
}

impl ShowSearch {
    pub fn judge(&self, release: Release) -> Candidate {
        let attributes = release::attributes(&release.title);

        match self.matcher.matches(&release.title) {
            Some(Match { episodes, batch, nonstandard }) => {
                let mut verdict = self.rules.judge(&release, &attributes, episodes.len() as u32, batch);
                verdict.nonstandard = nonstandard;

                if self.pinned
                    && let Some(i) = self.sources.iter().position(|s| s.name == release.source)
                {
                    verdict.score += (self.sources.len() - i) as i64 * 500;
                }

                Candidate { release, attributes, episodes, batch, verdict }
            },
            None => Candidate {
                release,
                attributes,
                episodes: Vec::new(),
                batch: false,
                verdict: Verdict {
                    accepted: false,
                    score: 0,
                    rejections: vec![
                        "Couldn't resolve this show's season and episode; choose them manually or search again".into(),
                    ],
                    warnings: Vec::new(),
                    nonstandard: false,
                },
            },
        }
    }
}

pub async fn search(
    state: &AppState,
    series_id: i64,
    season: u32,
    episodes: &[u32],
    query: Option<&str>,
) -> anyhow::Result<Vec<Candidate>> {
    let show = show_search(state, series_id).await?;

    if show.sources.is_empty() {
        bail!("there are no sources to search; add one in Settings → Sources");
    }

    let mut queries: Vec<String> = Vec::new();

    if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
        queries.push(q.to_string());
    } else {
        let titles = show.matcher.search_titles(Some(season));

        for t in titles.iter().take(2) {
            queries.push(t.clone());

            if let [e] = episodes {
                queries.push(format!("{t} {e:02}"));
            }
        }

        if let (Some(t), [e]) = (titles.first(), episodes) {
            queries.push(format!("{t} s{season:02}e{e:02}"));
        }

        queries.dedup();
    }

    let mut tasks = Vec::new();

    for source in &show.sources {
        let asks: Vec<Option<String>> =
            if Sources::can_search(source) { queries.iter().cloned().map(Some).collect() } else { vec![None] };

        for q in asks {
            let source = source.clone();

            tasks.push(async move {
                let result = match &q {
                    Some(q) => state.automation.sources.search(&source, q).await,
                    None => state.automation.sources.feed(&source).await,
                };

                (source.name.clone(), result)
            });
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut errors = Vec::new();

    for (name, result) in futures::future::join_all(tasks).await {
        match result {
            Ok(releases) => {
                for r in releases {
                    let key = r.info_hash.clone().unwrap_or_else(|| r.title.to_lowercase());

                    if seen.insert(key) {
                        out.push(show.judge(r));
                    }
                }
            },
            Err(e) => errors.push(format!("{name}: {e:#}")),
        }
    }

    if out.is_empty() && !errors.is_empty() {
        bail!("{}", errors.join("; "));
    }

    for e in errors {
        tracing::warn!("search: {e}");
    }

    sort(&mut out);
    Ok(out)
}

pub fn sort(c: &mut [Candidate]) {
    c.sort_by(|a, b| {
        b.verdict
            .accepted
            .cmp(&a.verdict.accepted)
            .then(a.verdict.nonstandard.cmp(&b.verdict.nonstandard))
            .then(b.verdict.score.cmp(&a.verdict.score))
            .then(b.release.seeders.cmp(&a.release.seeders))
    });
}

pub fn choose(candidates: &[Candidate], wanted: &HashSet<(u32, u32)>) -> Vec<(Candidate, Vec<(u32, u32)>)> {
    let covers =
        |c: &Candidate| -> Vec<(u32, u32)> { c.episodes.iter().filter(|e| wanted.contains(e)).copied().collect() };

    let ok: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| c.verdict.accepted && c.verdict.warnings.is_empty() && !covers(c).is_empty())
        .collect();

    let mut picks: Vec<(Candidate, Vec<(u32, u32)>)> = Vec::new();
    let mut left = wanted.clone();

    if wanted.len() >= 2
        && let Some(pack) = ok
            .iter()
            .filter(|c| c.batch && !c.verdict.nonstandard && covers(c).len() * 2 >= wanted.len().max(2))
            .max_by_key(|c| (covers(c).len(), c.verdict.score))
    {
        let got = covers(pack);

        for e in &got {
            left.remove(e);
        }

        picks.push(((*pack).clone(), got));
    }

    let mut remaining: Vec<(u32, u32)> = left.into_iter().collect();
    remaining.sort();

    for e in remaining {
        if picks.iter().any(|(_, got)| got.contains(&e)) {
            continue;
        }

        let best = ok
            .iter()
            .filter(|c| c.episodes.contains(&e))
            .max_by_key(|c| (!c.verdict.nonstandard, !c.batch, c.verdict.score));

        if let Some(c) = best {
            picks.push(((*c).clone(), covers(c)));
        }
    }

    picks
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grab {
    pub release: Release,
    pub series_id: Option<i64>,
    #[serde(default)]
    pub episodes: Vec<(u32, u32)>,
    #[serde(skip)]
    pub requested_by: Option<i64>,
}

pub async fn grab(state: &Arc<AppState>, g: Grab) -> anyhow::Result<i64> {
    let config = state.config.current();

    let series = match g.series_id {
        Some(id) => Some(series::get(state, id).await?),
        None => None,
    };

    let source = config.source(&g.release.source);
    let library = series.as_ref().and_then(|s| config.library(&s.library));

    let save_path = source
        .and_then(|s| s.download_path.clone())
        .or_else(|| library.and_then(|l| l.download_path.clone()))
        .and_then(|p| crate::paths::resolve_config_path(&p, state.config.config_dir()).ok())
        .unwrap_or_else(|| engine::download_root(state, &config));

    engine::ensure_dir(&save_path)?;

    let seeding: Seeding = series
        .as_ref()
        .and_then(|s| s.seeding.as_deref())
        .and_then(|j| serde_json::from_str(j).ok())
        .or_else(|| source.and_then(|s| s.seeding.clone()))
        .unwrap_or_else(|| config.downloads.seeding.clone());

    if g.requested_by.is_none() {
        let removed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM downloads WHERE state = 'removed' AND (link = ? OR (? IS NOT NULL AND hash = ?))",
        )
        .bind(&g.release.link)
        .bind(&g.release.info_hash)
        .bind(&g.release.info_hash)
        .fetch_one(&state.db)
        .await?;

        if removed > 0 {
            bail!("it was removed by hand before");
        }
    }

    let torrent = state.automation.sources.fetch(&g.release.link).await?;

    let (magnet, bytes) = match torrent {
        Torrent::Magnet(m) => (m, Vec::new()),
        Torrent::File(b) => (String::new(), b),
    };

    let params = libtorrent_sys::AddParams {
        magnet,
        torrent: bytes.clone(),
        resume: Vec::new(),
        save_path: save_path.to_string_lossy().to_string(),
        paused: false,
    };

    let hash = match state.automation.engine.add(&params) {
        Ok(h) => h,
        Err(e) if e.to_string().contains("exist") => bail!("that release is already downloading"),
        Err(e) => return Err(e).context("the torrent engine didn't take it"),
    };

    if let Some(existing) =
        sqlx::query_scalar::<_, i64>("SELECT id FROM downloads WHERE hash = ? AND state NOT IN ('removed', 'failed')")
            .bind(&hash)
            .fetch_optional(&state.db)
            .await?
    {
        return Ok(existing);
    }

    sqlx::query("DELETE FROM downloads WHERE hash = ?").bind(&hash).execute(&state.db).await?;

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO downloads (hash, name, series_id, episodes, source, link, size, save_path, torrent, seeding, requested_by, added_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&hash)
    .bind(&g.release.title)
    .bind(g.series_id)
    .bind(serde_json::to_string(&g.episodes)?)
    .bind(&g.release.source)
    .bind(&g.release.link)
    .bind(g.release.size)
    .bind(save_path.to_string_lossy().to_string())
    .bind((!bytes.is_empty()).then_some(bytes))
    .bind(serde_json::to_string(&seeding)?)
    .bind(g.requested_by)
    .bind(now())
    .fetch_one(&state.db)
    .await?;

    if let Some(series_id) = g.series_id {
        for (s, e) in &g.episodes {
            sqlx::query(
                "UPDATE episodes SET state = 'grabbed', download_id = ?, next_search = NULL
                 WHERE series_id = ? AND season = ? AND episode = ? AND state != 'done'",
            )
            .bind(id)
            .bind(series_id)
            .bind(s)
            .bind(e)
            .execute(&state.db)
            .await?;
        }

        state.events.send(Event::SeriesChanged { series_id });
    }

    tracing::info!("downloading {} from {}", g.release.title, g.release.source);
    state.events.send(Event::DownloadsChanged);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn search() -> ShowSearch {
        ShowSearch {
            matcher: Matcher::new(
                &["Show".into()],
                &[],
                BTreeMap::from([(1, Some(24)), (4, Some(24))]),
                matching::Numbering::Auto,
            ),
            rules: Rules::new(&crate::config::Profile::default(), &[]),
            sources: Vec::new(),
            pinned: false,
        }
    }

    fn release(title: &str) -> Release {
        Release {
            title: title.into(),
            source: "test".into(),
            link: title.into(),
            info_hash: None,
            size: None,
            seeders: Some(10),
            leechers: None,
            published: None,
            page: None,
        }
    }

    #[test]
    fn nonstandard_numbering_ranks_below_clear_numbering() {
        let search = search();
        let mut odd = search.judge(release("Show 4th_18 1080p"));
        assert_eq!(odd.episodes, vec![(4, 18)]);
        assert!(odd.verdict.accepted);
        assert!(odd.verdict.nonstandard);
        odd.verdict.score = i64::MAX;
        let clear = search.judge(release("Show S04E18 720p"));
        let mut candidates = vec![odd, clear];
        sort(&mut candidates);
        assert_eq!(candidates[0].release.title, "Show S04E18 720p");
        let picked = choose(&candidates, &HashSet::from([(4, 18)]));
        assert_eq!(picked[0].0.release.title, "Show S04E18 720p");
    }

    #[test]
    fn nonstandard_pack_is_only_a_fallback() {
        let search = search();
        let pack = search.judge(release("Show 4th_18-19 1080p"));
        let wanted = HashSet::from([(4, 18), (4, 19)]);

        let candidates =
            vec![pack.clone(), search.judge(release("Show S04E18 720p")), search.judge(release("Show S04E19 720p"))];

        let picked = choose(&candidates, &wanted);
        assert_eq!(picked.len(), 2);
        assert!(picked.iter().all(|(c, _)| !c.verdict.nonstandard));
        let picked = choose(&[pack], &wanted);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].1, vec![(4, 18), (4, 19)]);
    }

    #[test]
    fn unresolved_numbering_requires_manual_selection() {
        let candidate = search().judge(release("Show 4th_unknown_18"));
        assert!(candidate.episodes.is_empty());
        assert!(!candidate.verdict.accepted);
        assert!(candidate.verdict.rejections[0].contains("manually"));
        assert!(choose(&[candidate], &HashSet::from([(1, 18)])).is_empty());
    }
}
