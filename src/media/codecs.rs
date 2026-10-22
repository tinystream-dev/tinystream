// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ff::ffi;

pub fn video_codec_string(par: &ffi::AVCodecParameters) -> Option<String> {
    let extradata = extradata(par);

    match par.codec_id {
        ffi::AV_CODEC_ID_H264 => Some(avc(extradata).unwrap_or_else(|| {
            let profile = if par.profile > 0 { par.profile } else { 100 };
            let level = if par.level > 0 { par.level } else { 41 };
            format!("avc1.{profile:02x}00{level:02x}")
        })),
        ffi::AV_CODEC_ID_HEVC => Some(hevc(extradata).unwrap_or_else(|| {
            let profile = if par.profile > 0 { par.profile } else { 1 };
            let level = if par.level > 0 { par.level } else { 120 };

            let compat = match profile {
                1 => 6,
                2 => 4,
                _ => 0,
            };

            format!("hvc1.{profile}.{compat}.L{level}.B0")
        })),
        ffi::AV_CODEC_ID_AV1 => Some(av1(extradata).unwrap_or_else(|| "av01.0.08M.08".into())),
        ffi::AV_CODEC_ID_VP9 => Some("vp09.00.40.08".into()),
        _ => None,
    }
}

pub fn audio_codec_string(par: &ffi::AVCodecParameters) -> Option<String> {
    match par.codec_id {
        ffi::AV_CODEC_ID_AAC => {
            let aot = if par.profile >= 0 { par.profile + 1 } else { 2 };
            Some(format!("mp4a.40.{aot}"))
        },
        ffi::AV_CODEC_ID_MP3 => Some("mp4a.40.34".into()),
        ffi::AV_CODEC_ID_OPUS => Some("opus".into()),
        ffi::AV_CODEC_ID_FLAC => Some("flac".into()),
        ffi::AV_CODEC_ID_AC3 => Some("ac-3".into()),
        ffi::AV_CODEC_ID_EAC3 => Some("ec-3".into()),
        _ => None,
    }
}

fn extradata(par: &ffi::AVCodecParameters) -> &[u8] {
    if par.extradata.is_null() || par.extradata_size <= 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(par.extradata, par.extradata_size as usize) }
    }
}

fn avc(ed: &[u8]) -> Option<String> {
    if ed.len() >= 4 && ed[0] == 1 {
        return Some(format!("avc1.{:02x}{:02x}{:02x}", ed[1], ed[2], ed[3]));
    }

    let pos = ed.windows(4).position(|w| w[..3] == [0, 0, 1] && w[3] & 0x1F == 7)?;
    let sps = ed.get(pos + 4..pos + 7)?;
    Some(format!("avc1.{:02x}{:02x}{:02x}", sps[0], sps[1], sps[2]))
}

fn hevc(ed: &[u8]) -> Option<String> {
    if ed.len() < 13 || ed[0] != 1 {
        return None;
    }

    let space = ed[1] >> 6;
    let tier = (ed[1] >> 5) & 1;
    let profile = ed[1] & 0x1F;
    let compat = u32::from_be_bytes([ed[2], ed[3], ed[4], ed[5]]).reverse_bits();
    let constraints = &ed[6..12];
    let level = ed[12];
    let mut s = String::from("hvc1.");

    if space > 0 {
        s.push((b'A' + space - 1) as char);
    }

    s += &format!("{profile}.{compat:x}.{}{level}", if tier == 1 { 'H' } else { 'L' });
    let last = constraints.iter().rposition(|&b| b != 0);

    if let Some(last) = last {
        for b in &constraints[..=last] {
            s += &format!(".{b:x}");
        }
    }

    Some(s)
}

fn av1(ed: &[u8]) -> Option<String> {
    if ed.len() < 4 || ed[0] & 0x7F != 1 {
        return None;
    }

    let profile = ed[1] >> 5;
    let level = ed[1] & 0x1F;
    let tier = if ed[2] >> 7 == 1 { 'H' } else { 'M' };
    let high_bitdepth = (ed[2] >> 6) & 1 == 1;
    let twelve = (ed[2] >> 5) & 1 == 1;

    let depth = match (high_bitdepth, twelve) {
        (true, true) => 12,
        (true, false) => 10,
        _ => 8,
    };

    Some(format!("av01.{profile}.{level:02}{tier}.{depth:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hevc_main10() {
        let ed = [1u8, 0x02, 0x20, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 153];
        assert_eq!(hevc(&ed).unwrap(), "hvc1.2.4.L153.90");
    }

    #[test]
    fn avc_high() {
        assert_eq!(avc(&[1, 0x64, 0x00, 0x28]).unwrap(), "avc1.640028");
    }
}
