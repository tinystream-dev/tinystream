// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{Grab, ShowSearch, choose, series, show_search};
use crate::config::{RetryStep, Span};
use crate::db::now;
use crate::events::Event;
use crate::notifications;
use crate::state::AppState;

pub fn next_search(air_at: Option<i64>, wanted_at: i64, now: i64, steps: &[RetryStep]) -> Option<i64> {
    let window = steps.iter().map(|s| s.until.as_secs() as i64).max().unwrap_or(0);
    let fresh = air_at.is_some_and(|a| wanted_at - a < window);
    let backfill = [RetryStep { every: Span(Duration::from_secs(86400)), until: Span(Duration::from_secs(7 * 86400)) }];
    let (base, steps) = if fresh { (air_at.unwrap(), steps) } else { (wanted_at, &backfill[..]) };
    let elapsed = now - base;
    steps.iter().find(|s| elapsed < s.until.as_secs() as i64).map(|s| now + s.every.as_secs() as i64)
}

async fn sync_states(state: &AppState) -> anyhow::Result<()> {
    let db = &state.db;
    let t = now();

    let have = "EXISTS (SELECT 1 FROM media m JOIN items i ON i.id = m.item_id JOIN series s ON s.path = i.path
                 WHERE s.id = episodes.series_id AND m.season = episodes.season
                   AND episodes.episode BETWEEN m.episode AND COALESCE(m.episode_end, m.episode))";

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE episodes SET state = 'done', next_search = NULL WHERE state != 'done' AND {have}"
    )))
    .execute(db)
    .await?;

    sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE episodes SET state = 'idle' WHERE state = 'done' AND NOT {have}")))
        .execute(db)
        .await?;

    sqlx::query(
        "UPDATE episodes SET state = 'wanted', download_id = NULL, next_search = ?
         WHERE state = 'grabbed' AND (download_id IS NULL OR download_id IN (SELECT id FROM downloads WHERE state IN ('failed', 'removed')))",
    )
    .bind(t)
    .execute(db)
    .await?;

    sqlx::query(
        "UPDATE episodes SET state = 'wanted', wanted_at = ?1, attempts = 0,
            next_search = CASE WHEN air_at > ?1 THEN air_at WHEN aired = 1 OR air_at IS NOT NULL THEN ?1 ELSE NULL END
         WHERE state = 'idle' AND season > 0 AND series_id IN (
            SELECT id FROM series s WHERE s.monitor = 'missing'
               OR (s.monitor = 'future' AND (episodes.air_at >= s.monitored_at - 3600 OR (episodes.air_at IS NULL AND episodes.aired = 0))))",
    )
    .bind(t)
    .execute(db)
    .await?;

    sqlx::query(
        "UPDATE episodes SET next_search = CASE WHEN air_at > ?1 THEN air_at ELSE ?1 END
         WHERE state = 'wanted' AND next_search IS NULL AND (aired = 1 OR air_at IS NOT NULL)",
    )
    .bind(t)
    .execute(db)
    .await?;

    Ok(())
}

const ANNOUNCE_WITHIN: i64 = 6 * 3600;

async fn announce_aired(state: &AppState) -> anyhow::Result<()> {
    let t = now();

    let fresh: Vec<(i64, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT e.series_id, e.season, e.episode, e.title FROM episodes e JOIN series s ON s.id = e.series_id
         WHERE e.announced_at IS NULL AND e.season > 0 AND s.monitor != 'none' AND e.air_at BETWEEN ?1 - ?2 AND ?1
         ORDER BY e.series_id, e.season, e.episode",
    )
    .bind(t)
    .bind(ANNOUNCE_WITHIN)
    .fetch_all(&state.db)
    .await?;

    sqlx::query("UPDATE episodes SET announced_at = ?1 WHERE announced_at IS NULL AND (aired = 1 OR air_at <= ?1)")
        .bind(t)
        .execute(&state.db)
        .await?;

    let mut shows: Vec<(i64, Vec<(u32, u32)>, Option<String>)> = Vec::new();

    for (series_id, season, episode, title) in fresh {
        match shows.last_mut() {
            Some((id, eps, _)) if *id == series_id => eps.push((season as u32, episode as u32)),
            _ => shows.push((series_id, vec![(season as u32, episode as u32)], title)),
        }
    }

    for (series_id, episodes, title) in shows {
        let show = series::get(state, series_id).await?;
        let item_id = series::item_id(state, &show.path).await?;
        let label = notifications::episodes_label(&episodes);
        let one = episodes.len() == 1;

        let body = match (one, title) {
            (true, Some(t)) => format!("{label} · {t}"),
            _ => label,
        };

        let users = notifications::who_can_see(state, &show.library).await;

        notifications::send(
            state,
            &users,
            notifications::New {
                kind: "aired",
                priority: true,
                title: format!("New {} of {} aired", if one { "episode" } else { "episodes" }, show.title),
                body: Some(body),
                image: item_id.map(|id| format!("/api/images/item/{id}/poster")).or(show.poster),
                link: Some(item_id.map(|id| format!("/title/{id}")).unwrap_or_else(|| "/calendar".into())),
                ..Default::default()
            },
        )
        .await;
    }

    Ok(())
}

async fn refresh_schedules(state: &Arc<AppState>) -> anyhow::Result<()> {
    let t = now();

    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM series WHERE provider_id IS NOT NULL AND (schedule_at IS NULL OR (monitor != 'none'
            AND ((COALESCE(status, 'airing') IN ('airing', 'upcoming', 'hiatus') AND schedule_at < ?1 - 12 * 3600)
                OR schedule_at < ?1 - 7 * 86400)))
         ORDER BY schedule_at LIMIT 3",
    )
    .bind(t)
    .fetch_all(&state.db)
    .await?;

    for id in due {
        if let Err(e) = series::refresh_schedule(state, id).await {
            tracing::warn!("refreshing the schedule of show {id}: {e:#}");

            sqlx::query("UPDATE series SET schedule_at = ? WHERE id = ?")
                .bind(t - 11 * 3600)
                .bind(id)
                .execute(&state.db)
                .await?;
        }
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct Due {
    series_id: i64,
    season: i64,
    episode: i64,
    air_at: Option<i64>,
    wanted_at: Option<i64>,
}

async fn grab_picks(
    state: &Arc<AppState>,
    series_id: i64,
    picks: Vec<(super::Candidate, Vec<(u32, u32)>)>,
) -> HashSet<(u32, u32)> {
    let mut got = HashSet::new();

    for (candidate, episodes) in picks {
        let title = candidate.release.title.clone();

        if got.iter().any(|e| episodes.contains(e)) {
            continue;
        }

        match super::grab(
            state,
            Grab {
                release: candidate.release,
                series_id: Some(series_id),
                episodes: episodes.clone(),
                requested_by: None,
            },
        )
        .await
        {
            Ok(_) => got.extend(episodes),
            Err(e) => tracing::warn!("couldn't download {title}: {e:#}"),
        }
    }

    got
}

async fn run_searches(state: &Arc<AppState>) -> anyhow::Result<()> {
    let t = now();

    let due: Vec<Due> = sqlx::query_as(
        "SELECT e.series_id, e.season, e.episode, e.air_at, e.wanted_at FROM episodes e JOIN series s ON s.id = e.series_id
         WHERE e.state = 'wanted' AND e.next_search <= ?1 AND s.monitor != 'none'
           AND (e.aired = 1 OR (e.air_at IS NOT NULL AND e.air_at <= ?1))
         ORDER BY e.next_search LIMIT 60",
    )
    .bind(t)
    .fetch_all(&state.db)
    .await?;

    if due.is_empty() {
        return Ok(());
    }

    let config = state.config.current();
    let mut groups: HashMap<(i64, i64), Vec<Due>> = HashMap::new();

    for d in due {
        groups.entry((d.series_id, d.season)).or_default().push(d);
    }

    for ((series_id, season), eps) in groups {
        let numbers: Vec<u32> = eps.iter().map(|e| e.episode as u32).collect();
        let wanted: HashSet<(u32, u32)> = numbers.iter().map(|e| (season as u32, *e)).collect();

        let got =
            match super::search(state, series_id, season as u32, if numbers.len() == 1 { &numbers } else { &[] }, None)
                .await
            {
                Ok(candidates) => grab_picks(state, series_id, choose(&candidates, &wanted)).await,
                Err(e) => {
                    tracing::warn!("searching for show {series_id} season {season}: {e:#}");
                    HashSet::new()
                },
            };

        for e in eps {
            if got.contains(&(season as u32, e.episode as u32)) {
                continue;
            }

            let next = next_search(e.air_at, e.wanted_at.unwrap_or(t), t, &config.automation.retry);

            sqlx::query(
                "UPDATE episodes SET attempts = attempts + 1, searched_at = ?, next_search = ?,
                    state = CASE WHEN ? IS NULL THEN 'missing' ELSE state END
                 WHERE series_id = ? AND season = ? AND episode = ? AND state = 'wanted'",
            )
            .bind(t)
            .bind(next)
            .bind(next)
            .bind(series_id)
            .bind(season)
            .bind(e.episode)
            .execute(&state.db)
            .await?;
        }

        state.events.send(Event::SeriesChanged { series_id });
    }

    Ok(())
}

async fn rss_sync(state: &Arc<AppState>) -> anyhow::Result<()> {
    let config = state.config.current();

    let waiting: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT e.series_id, e.season, e.episode FROM episodes e JOIN series s ON s.id = e.series_id
         WHERE e.state IN ('wanted', 'missing') AND s.monitor != 'none'
           AND (e.aired = 1 OR (e.air_at IS NOT NULL AND e.air_at <= ?))",
    )
    .bind(now() + 3600)
    .fetch_all(&state.db)
    .await?;

    if waiting.is_empty() {
        return Ok(());
    }

    let mut wanted: HashMap<i64, HashSet<(u32, u32)>> = HashMap::new();

    for (s, season, e) in waiting {
        wanted.entry(s).or_default().insert((season as u32, e as u32));
    }

    let mut shows: Vec<(i64, ShowSearch)> = Vec::new();

    for &id in wanted.keys() {
        match show_search(state, id).await {
            Ok(s) => shows.push((id, s)),
            Err(e) => tracing::debug!("show {id}: {e:#}"),
        }
    }

    let feeds = futures::future::join_all(
        config
            .sources
            .iter()
            .filter(|s| s.enabled)
            .map(|s| async move { (s.name.clone(), state.automation.sources.feed(s).await) }),
    )
    .await;

    let mut found: HashMap<i64, Vec<super::Candidate>> = HashMap::new();

    for (name, result) in feeds {
        let releases = match result {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("reading {name}'s feed: {e:#}");
                continue;
            },
        };

        for release in releases {
            for (id, show) in &shows {
                if !show.sources.iter().any(|s| s.name == name) {
                    continue;
                }

                let c = show.judge(release.clone());

                if c.episodes.iter().any(|e| wanted[id].contains(e)) {
                    found.entry(*id).or_default().push(c);
                }
            }
        }
    }

    for (id, mut candidates) in found {
        super::sort(&mut candidates);
        let got = grab_picks(state, id, choose(&candidates, &wanted[&id])).await;

        if !got.is_empty() {
            tracing::info!("found {} episode(s) of show {id} in feeds", got.len());
        }
    }

    Ok(())
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut last_rss: Option<Instant> = None;

        loop {
            let config = state.config.current();

            if let Err(e) = refresh_schedules(&state).await {
                tracing::warn!("schedules: {e:#}");
            }

            if let Err(e) = sync_states(&state).await {
                tracing::error!("episode states: {e:#}");
            }

            if let Err(e) = announce_aired(&state).await {
                tracing::warn!("announcing new episodes: {e:#}");
            }

            if let Err(e) = run_searches(&state).await {
                tracing::error!("searches: {e:#}");
            }

            if !config.sources.is_empty() && last_rss.is_none_or(|t| t.elapsed() >= *config.automation.rss_interval) {
                last_rss = Some(Instant::now());

                if let Err(e) = rss_sync(&state).await {
                    tracing::error!("feeds: {e:#}");
                }
            }

            let next: Option<i64> = sqlx::query_scalar("SELECT MIN(next_search) FROM episodes WHERE state = 'wanted'")
                .fetch_one(&state.db)
                .await
                .ok()
                .flatten();

            let wait = next.map(|n| (n - now()).clamp(1, 30) as u64).unwrap_or(30);

            tokio::select! {
                _ = state.automation.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Automation;

    #[test]
    fn retries_follow_the_schedule() {
        let steps = Automation::default().retry;
        let air = 1_000_000;

        assert_eq!(next_search(Some(air), air, air + 60, &steps), Some(air + 60 + 120));

        assert_eq!(next_search(Some(air), air, air + 3600, &steps), Some(air + 3600 + 300));

        assert_eq!(next_search(Some(air), air, air + 8 * 86400, &steps), None);

        let wanted = air + 30 * 86400;
        assert_eq!(next_search(Some(air), wanted, wanted + 10, &steps), Some(wanted + 10 + 86400));
    }
}
