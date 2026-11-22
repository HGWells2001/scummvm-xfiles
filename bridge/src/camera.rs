use xf::front::{self, host, Session};
use xf::front::light::{CAMERA_EXPOSURES, HELD_CAMERA};
use xf::world::devices::CAMERA_OWNER;

pub(super) const SHUTTER: u32 = 0x87bb;
pub(super) const EMPTY_SHUTTER: u32 = 0xdf10;

#[derive(Default)]
pub(super) struct Camera {
    background: Option<(u32, u64)>,
    shutter: Option<(u32, u64)>,
}

impl Camera {
    pub(super) fn update_background(&mut self, s: &Session<'_>) {
        if self.background.map(|(asset, _)| asset) != s.r.background {
            self.background = s.r.background.map(|asset| (asset, s.now));
        }
    }

    pub(super) fn press(&mut self, s: &mut Session<'_>, mix: &mut host::Mix, point: (i32, i32)) -> bool {
        if s.r.w.dev.held != Some(HELD_CAMERA) || !s.r.w.dev.held_overlay
            || !s.r.game_running || s.intro_playing() || s.r.game_paused()
            || s.r.frozen || s.r.rewind_box || s.alt.paused || s.alt.msgbox.is_some()
            || !s.r.hit(point.0, point.1).is_some_and(|hit| hit.owner == CAMERA_OWNER)
        {
            return false;
        }
        let count = s.r.w.pc.camera;
        let photos = s.r.w.pc.photos.len();
        let source = (count < CAMERA_EXPOSURES).then(|| self.photo_source(s)).flatten();
        // The original hit handler still fires scripted reactions to the camera.
        host::button(s, host::Button::Left, Some(point));
        let sound = if count >= CAMERA_EXPOSURES {
            EMPTY_SHUTTER
        } else if s.r.w.pc.camera > count {
            let pc = &mut s.r.w.pc;
            pc.photos.truncate(photos);
            pc.photo_frames.resize(photos, None);
            if let Some((asset, position)) = source {
                let left = (point.0 - 160).clamp(0, 280) as i16;
                pc.photos.push((asset, false));
                pc.photo_frames.push(Some((position, [left, 0, left + 320, 240])));
                SHUTTER
            } else {
                pc.camera = count;
                EMPTY_SHUTTER
            }
        } else {
            return true;
        };
        self.play_shutter(mix, sound, s.now);
        true
    }

    pub(super) fn play_shutter(&mut self, mix: &mut host::Mix, sound: u32, now: u64) {
        mix.voices.retain(|voice| voice.asset != SHUTTER && voice.asset != EMPTY_SHUTTER);
        self.shutter = Some((sound, now));
    }

    fn photo_source(&mut self, s: &mut Session<'_>) -> Option<(u32, u32)> {
        self.update_background(s);
        let background = self.background.map(|(asset, start)| {
            (asset, s.picture_now().saturating_sub(start))
        });
        for (asset, elapsed) in s.picture_head().into_iter().chain(background) {
            let Some(clip) = s.clip(asset) else { continue };
            if clip.ext() == "pff" {
                if s.picture(asset).is_some() {
                    return Some((asset, 0));
                }
            } else if matches!(clip.ext().as_str(), "xmv" | "mov" | "nmv" | "dmv") {
                let position = if clip.is_still() { 0 } else { elapsed };
                // The album resolves the native VCPhoto timestamp in 1/600 s.
                let ticks = (position.saturating_mul(600) / 1000).min(u64::from(u32::MAX));
                return Some((asset, ticks as u32));
            }
        }
        None
    }

    pub(super) fn audio(&mut self, s: &mut Session<'_>) -> Option<front::Cue> {
        let (asset, start) = self.shutter?;
        let cue = (!s.r.game_paused() && !s.r.frozen && !s.r.rewind_box
            && !s.alt.paused && s.alt.msgbox.is_none())
            .then(|| super::reaction_cue(s, asset, s.now.saturating_sub(start))).flatten();
        if cue.is_none() {
            self.shutter = None;
        }
        cue
    }
}
