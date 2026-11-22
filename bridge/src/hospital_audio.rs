use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
use xf::{
    front::{Cue, Session},
    media::{audio::{self, Pcm}, qt::{Handler, Movie}},
};

// These hospital movies carry separate, enabled dialogue tracks on the Italian CDs.
// The same assets on the English CDs already contain the complete single-track mix.
pub const SCULLY_DISCOVERY: u32 = 0x1561b;
pub const SCULLY_REPLY: u32 = 0x53ca;

#[derive(Default)]
pub struct HospitalAudio {
    movies: HashMap<PathBuf, Option<Arc<Pcm>>>,
}

impl HospitalAudio {
    pub fn mix_dialogue(&mut self, session: &mut Session<'_>, cues: &mut Vec<Cue>) {
        if session.intro_playing() || session.r.game_paused() || session.r.frozen
            || session.r.rewind_box || session.alt.msgbox.is_some() || session.preview.is_some()
        {
            return;
        }
        let Some((asset, elapsed)) = session.picture_head() else { return };
        if !matches!(asset, SCULLY_DISCOVERY | SCULLY_REPLY) {
            return;
        }
        let Some(clip) = session.clip(asset) else { return };
        let path = session.lang_file(&clip, session.game_lang);
        let Some(pcm) = self.movies.entry(path.clone()).or_insert_with(|| decode_tracks(&path)) else { return };
        let frames = |ms: u64| (ms * u64::from(pcm.rate) / 1000) as usize;
        let start = frames(clip.start.into());
        let end = if clip.end > clip.start { frames(clip.end.into()).min(pcm.frames()) } else { pcm.frames() };
        let primary = cues.iter_mut().find(|cue| cue.asset == asset);
        // Keep the normal audio clock while the first track is active. Once it ends,
        // the playing picture still provides the position for the second track's tail.
        let pos = primary.as_ref().map_or_else(|| start + frames(elapsed), |cue| cue.pos);
        if pos < end {
            let cue = Cue { asset, pcm: Arc::clone(pcm), pos, start, end, looped: false };
            if let Some(primary) = primary {
                *primary = cue;
            } else {
                cues.push(cue);
            }
        }
    }
}

fn decode_tracks(path: &Path) -> Option<Arc<Pcm>> {
    let mut movie = Movie::open_header(path).ok()?;
    movie.tracks.retain(|track| track.handler == Handler::Sound && track.enabled);
    if movie.tracks.len() < 2 {
        return None;
    }
    let bytes = xf::io::read(path).ok()?;
    let tracks = std::mem::take(&mut movie.tracks);
    let mut mixed: Option<Pcm> = None;
    for track in tracks {
        // The existing decoder places one track on the movie timeline using its edit list.
        movie.tracks.push(track);
        let pcm = audio::decode(&movie, &bytes).ok()?;
        movie.tracks.clear();
        if let Some(mixed) = mixed.as_mut() {
            if (mixed.rate, mixed.channels) != (pcm.rate, pcm.channels) {
                return None;
            }
            mixed.samples.resize(mixed.samples.len().max(pcm.samples.len()), 0);
            for (out, sample) in mixed.samples.iter_mut().zip(pcm.samples) {
                *out = out.saturating_add(sample);
            }
        } else {
            mixed = Some(pcm);
        }
    }
    mixed.map(Arc::new)
}
