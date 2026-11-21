use std::collections::HashMap;
use xf::{media::hot::Hotspot, world::moviehot::relative_to_start};

#[derive(Default)]
pub struct RestoredHotspots {
    original: Option<(u32, Vec<Hotspot>)>,
}

impl RestoredHotspots {
    pub fn update(
        &mut self,
        hotspots: &mut HashMap<u32, Vec<Hotspot>>,
        shown: Option<(u32, u64)>,
        now: u64,
        media_changes: &[(u64, u32, u32)],
    ) {
        if let Some((asset, _)) = &self.original {
            // Put the windows back when playback stops or the same asset restarts.
            if shown.is_none_or(|(shown, _)| shown != *asset)
                || media_changes.iter().any(|&(_, changed, _)| changed == *asset)
            {
                if let Some((asset, original)) = self.original.take() {
                    hotspots.insert(asset, original);
                }
            }
        }
        let Some((asset, position)) = shown else { return; };
        let Some(&(started, _, 1)) = media_changes.iter().rev().find(|&&(_, changed, _)| changed == asset) else {
            return;
        };
        // The first rendered position includes the saved seek, but the hit clock
        // measures only time since the media-start event.
        let seek = position.saturating_sub(now.saturating_sub(started));
        if seek == 0 {
            return;
        }
        if let Some(windows) = hotspots.get_mut(&asset).filter(|windows| !windows.is_empty()) {
            let shifted = relative_to_start(windows, seek.min(u64::from(u32::MAX)) as u32);
            self.original = Some((asset, std::mem::replace(windows, shifted)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_clip_uses_saved_position_and_normal_replay_restores_windows() {
        let window = |t0, t1| Hotspot { id: 600, rect: [10, 20, 30, 40], t0, t1 };
        let original = vec![window(10, 50), window(80, 160), window(250, 500)];
        let mut hotspots = HashMap::from([(42, original.clone())]);
        let mut restored = RestoredHotspots::default();
        restored.update(&mut hotspots, Some((42, 120)), 1020, &[(1000, 42, 1)]);
        assert_eq!(hotspots[&42], [window(0, 60), window(150, 400)]);
        restored.update(&mut hotspots, Some((42, 130)), 1030, &[]);
        assert_eq!(hotspots[&42], [window(0, 60), window(150, 400)]);
        restored.update(&mut hotspots, Some((42, 0)), 1100, &[(1090, 42, 0), (1100, 42, 1)]);
        assert_eq!(hotspots[&42], original);
    }
}
