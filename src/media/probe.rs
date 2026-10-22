// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;

use super::codecs;
use super::ff::{self, Input, ffi};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub duration: Option<f64>,
    pub video: Option<VideoTrack>,
    pub audio: Vec<AudioTrack>,
    pub subtitles: Vec<SubtitleTrack>,
    pub fonts: Vec<Font>,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct VideoTrack {
    pub index: usize,
    pub codec: String,

    pub codec_string: Option<String>,
    pub width: i32,
    pub height: i32,
    pub fps: f64,
    pub bit_depth: i32,
    pub hdr: bool,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub index: usize,
    pub codec: String,
    pub codec_string: Option<String>,
    pub channels: i32,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub id: String,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,

    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct Font {
    pub index: usize,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub start: f64,
    pub end: f64,
    pub title: Option<String>,
}

pub const TEXT_SUBTITLE_CODECS: &[ffi::AVCodecID] = &[
    ffi::AV_CODEC_ID_ASS,
    ffi::AV_CODEC_ID_SSA,
    ffi::AV_CODEC_ID_SUBRIP,
    ffi::AV_CODEC_ID_TEXT,
    ffi::AV_CODEC_ID_WEBVTT,
    ffi::AV_CODEC_ID_MOV_TEXT,
];

fn codec_name(id: ffi::AVCodecID) -> String {
    unsafe { ff::opt_str(ffi::avcodec_get_name(id)) }.unwrap_or_else(|| "unknown".into())
}

fn is_font(filename: &str, mimetype: Option<&str>) -> bool {
    let m = mimetype.unwrap_or_default().to_ascii_lowercase();
    let f = filename.to_ascii_lowercase();

    m.contains("font")
        || m.contains("truetype")
        || m.contains("opentype")
        || [".ttf", ".otf", ".ttc", ".woff", ".woff2"].iter().any(|e| f.ends_with(e))
}

pub fn probe(path: &Path) -> anyhow::Result<MediaInfo> {
    let input = Input::open(path, true)?;
    let ctx = unsafe { &*input.0 };

    let duration = (ctx.duration != ffi::AV_NOPTS_VALUE && ctx.duration > 0)
        .then(|| ctx.duration as f64 / ffi::AV_TIME_BASE as f64);

    let mut info = MediaInfo {
        duration,
        video: None,
        audio: Vec::new(),
        subtitles: Vec::new(),
        fonts: Vec::new(),
        chapters: Vec::new(),
    };

    let best_video =
        unsafe { ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_VIDEO, -1, -1, std::ptr::null_mut(), 0) };

    for (index, &st) in input.streams().iter().enumerate() {
        let st = unsafe { &*st };
        let par = unsafe { &*st.codecpar };
        let meta = st.metadata;
        let language = unsafe { ff::dict_get(meta, "language") }.filter(|l| l != "und");
        let title = unsafe { ff::dict_get(meta, "title") };
        let default = st.disposition & ffi::AV_DISPOSITION_DEFAULT as i32 != 0;

        match par.codec_type {
            ffi::AVMEDIA_TYPE_VIDEO if index as i32 == best_video => {
                let desc = unsafe { ffi::av_pix_fmt_desc_get(par.format) };
                let bit_depth = if desc.is_null() { 8 } else { unsafe { (*desc).comp[0].depth } };
                let fps = ff::q2d(if st.avg_frame_rate.num > 0 { st.avg_frame_rate } else { st.r_frame_rate });

                info.video = Some(VideoTrack {
                    index,
                    codec: codec_name(par.codec_id),
                    codec_string: codecs::video_codec_string(par),
                    width: par.width,
                    height: par.height,
                    fps,
                    bit_depth,
                    hdr: matches!(par.color_trc, ffi::AVCOL_TRC_SMPTE2084 | ffi::AVCOL_TRC_ARIB_STD_B67),
                });
            },
            ffi::AVMEDIA_TYPE_AUDIO => info.audio.push(AudioTrack {
                index,
                codec: codec_name(par.codec_id),
                codec_string: codecs::audio_codec_string(par),
                channels: par.ch_layout.nb_channels,
                language,
                title,
                default,
            }),
            ffi::AVMEDIA_TYPE_SUBTITLE => info.subtitles.push(SubtitleTrack {
                id: format!("s{index}"),
                codec: codec_name(par.codec_id),
                language,
                title,
                default,
                forced: st.disposition & ffi::AV_DISPOSITION_FORCED as i32 != 0,
                supported: TEXT_SUBTITLE_CODECS.contains(&par.codec_id),
            }),
            ffi::AVMEDIA_TYPE_ATTACHMENT => {
                let filename = unsafe { ff::dict_get(meta, "filename") }.unwrap_or_default();
                let mimetype = unsafe { ff::dict_get(meta, "mimetype") };

                if is_font(&filename, mimetype.as_deref()) && par.extradata_size > 0 {
                    info.fonts.push(Font { index, filename });
                }
            },
            _ => {},
        }
    }

    if !info.audio.iter().any(|a| a.default)
        && let Some(first) = info.audio.first_mut()
    {
        first.default = true;
    }

    for &ch in input.chapters() {
        let ch = unsafe { &*ch };
        let tb = ch.time_base;

        info.chapters.push(Chapter {
            start: ch.start as f64 * ff::q2d(tb),
            end: ch.end as f64 * ff::q2d(tb),
            title: unsafe { ff::dict_get(ch.metadata, "title") },
        });
    }

    info.subtitles.extend(super::subs::sidecars(path));
    Ok(info)
}

pub fn attachment(path: &Path, index: usize) -> anyhow::Result<(String, Vec<u8>)> {
    let input = Input::open(path, false)?;
    let st = input.stream(index).ok_or_else(|| anyhow::anyhow!("no stream #{index}"))?;
    let par = unsafe { &*st.codecpar };

    if par.codec_type != ffi::AVMEDIA_TYPE_ATTACHMENT || par.extradata.is_null() {
        anyhow::bail!("stream #{index} isn't an attachment");
    }

    let data = unsafe { std::slice::from_raw_parts(par.extradata, par.extradata_size as usize) }.to_vec();
    let filename = unsafe { ff::dict_get(st.metadata, "filename") }.unwrap_or_else(|| format!("font-{index}"));
    Ok((filename, data))
}

pub fn duration(path: &Path) -> anyhow::Result<Option<f64>> {
    let input = Input::open(path, false)?;
    let d = unsafe { (*input.0).duration };
    Ok((d != ffi::AV_NOPTS_VALUE && d > 0).then(|| d as f64 / ffi::AV_TIME_BASE as f64))
}

pub fn fill_missing_durations(state: Arc<AppState>, library: String) {
    tokio::spawn(async move {
        let rows: Vec<(i64, String)> = match sqlx::query_as(
            "SELECT m.id, m.path FROM media m JOIN items i ON i.id = m.item_id
             WHERE i.library = ? AND m.duration IS NULL",
        )
        .bind(&library)
        .fetch_all(&state.db)
        .await
        {
            Ok(r) => r,
            Err(e) => return tracing::warn!("{e}"),
        };

        if rows.is_empty() {
            return;
        }

        tracing::debug!("reading durations of {} files in {library:?}", rows.len());
        let sem = Arc::new(tokio::sync::Semaphore::new(4));
        let mut tasks = Vec::new();

        for (id, path) in rows {
            let permit = sem.clone().acquire_owned().await;
            let state = state.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let p = path.clone();

                match tokio::task::spawn_blocking(move || duration(Path::new(&p))).await {
                    Ok(Ok(Some(d))) => {
                        let _ = sqlx::query("UPDATE media SET duration = ? WHERE id = ?")
                            .bind(d)
                            .bind(id)
                            .execute(&state.db)
                            .await;
                    },
                    Ok(Ok(None)) => {},
                    Ok(Err(e)) => tracing::warn!("can't read {path}: {e:#}"),
                    Err(_) => {},
                }
            }));
        }

        for t in tasks {
            let _ = t.await;
        }
    });
}
