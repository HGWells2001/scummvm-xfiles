use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use xf::front::Session;
use xf::media::pict::Picture;

// PhotoDownload in the original PC album's Download button.
const DOWNLOAD_ACTION: u32 = 0xd03d;
const FOLDER: &str = "foto";
type Photo = (u32, Option<(u32, [i16; 4])>);

pub(super) struct AlbumBefore {
    actions: usize,
    downloaded: HashMap<Photo, usize>,
}

impl AlbumBefore {
    pub(super) fn capture(s: &Session<'_>) -> Option<Self> {
        let pc = &s.r.w.pc;
        if !pc.album_open || pc.camera == 0 {
            return None;
        }
        let mut downloaded = HashMap::new();
        for photo in downloaded_photos(s) {
            *downloaded.entry(photo).or_default() += 1;
        }
        Some(Self { actions: s.r.execs.len(), downloaded })
    }

    pub(super) fn export(mut self, s: &mut Session<'_>, saves: &Path) -> Option<String> {
        if !s.r.execs.get(self.actions..)?.contains(&DOWNLOAD_ACTION) {
            return None;
        }
        let new_photos: Vec<_> = downloaded_photos(s).filter(|photo| {
            let count = self.downloaded.entry(*photo).or_default();
            if *count == 0 {
                true
            } else {
                *count -= 1;
                false
            }
        }).collect();
        if new_photos.is_empty() {
            return None;
        }
        let folder = saves.join(FOLDER);
        let create = fs::create_dir_all(&folder).and_then(|()| {
            if folder.canonicalize()?.parent() == Some(saves) {
                Ok(())
            } else {
                Err(io::Error::other("Photo folder is outside the save directory"))
            }
        });
        if let Err(error) = create {
            eprintln!("XFILES: cannot create {}: {error}", folder.display());
            return Some("Impossibile creare la cartella foto nei salvataggi di ScummVM.".into());
        }
        let batch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let mut written = 0;
        for (index, (asset, frame)) in new_photos.iter().copied().enumerate() {
            let result = s.photo_picture(asset, frame)
                .ok_or_else(|| io::Error::other("Original photo could not be decoded"))
                .and_then(|picture| write_bmp(&folder, batch, index + 1, &picture));
            match result {
                Ok(()) => written += 1,
                Err(error) => eprintln!("XFILES: cannot export photo {asset:#x}: {error}"),
            }
        }
        if written > 0 {
            eprintln!("XFILES: exported {written} photos to {}", folder.display());
        }
        let total = new_photos.len();
        Some(if written == total {
            format!("Foto salvate sul PC ({written}): xfiles-shiloh/foto nei salvataggi di ScummVM.")
        } else {
            format!("Foto salvate sul PC: {written} di {total}. Cartella xfiles-shiloh/foto.")
        })
    }
}

fn downloaded_photos<'s>(s: &'s Session<'_>) -> impl Iterator<Item = Photo> + 's {
    let pc = &s.r.w.pc;
    pc.photos.iter().enumerate().filter(|(_, (_, downloaded))| *downloaded)
        .map(|(index, &(asset, _))| (asset, pc.photo_frames.get(index).copied().flatten()))
}

fn write_bmp(folder: &Path, batch: u128, index: usize, picture: &Picture) -> io::Result<()> {
    let bytes = bmp_bytes(picture)?;
    let mut suffix = 0u64;
    loop {
        let name = if suffix == 0 {
            format!("XFiles-{batch}-{index:02}.bmp")
        } else {
            format!("XFiles-{batch}-{index:02}-{suffix}.bmp")
        };
        let path = folder.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let result = file.write_all(&bytes).and_then(|()| file.sync_all());
                drop(file);
                if result.is_err() {
                    let _ = fs::remove_file(path);
                }
                return result;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => suffix += 1,
            Err(error) => return Err(error),
        }
    }
}

fn bmp_bytes(picture: &Picture) -> io::Result<Vec<u8>> {
    const HEADER_LEN: u32 = 54;
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "Invalid RGB picture");
    let width = i32::try_from(picture.width).map_err(|_| invalid())?;
    let height = i32::try_from(picture.height).map_err(|_| invalid())?;
    let row_bytes = picture.width.checked_mul(3).ok_or_else(invalid)?;
    if width == 0 || height == 0 || row_bytes.checked_mul(picture.height) != Some(picture.rgb.len()) {
        return Err(invalid());
    }
    let padding = (4 - row_bytes % 4) % 4;
    let pixel_bytes = (row_bytes + padding).checked_mul(picture.height)
        .and_then(|n| u32::try_from(n).ok()).ok_or_else(invalid)?;
    let file_len = HEADER_LEN.checked_add(pixel_bytes).ok_or_else(invalid)?;
    let mut bmp = Vec::with_capacity(file_len as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_len.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&HEADER_LEN.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
    bmp.extend_from_slice(&width.to_le_bytes());
    bmp.extend_from_slice(&(-height).to_le_bytes()); // Top-down rows
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]); // Uncompressed RGB
    bmp.extend_from_slice(&pixel_bytes.to_le_bytes());
    bmp.extend_from_slice(&[0; 16]);
    for row in picture.rgb.chunks_exact(row_bytes) {
        for rgb in row.chunks_exact(3) {
            bmp.extend_from_slice(&[rgb[2], rgb[1], rgb[0]]);
        }
        bmp.extend_from_slice(&[0; 3][..padding]);
    }
    Ok(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_stores_top_down_bgr_rows_with_padding() {
        let picture = Picture {
            width: 1, height: 2, rgb: vec![1, 2, 3, 4, 5, 6], alpha: None,
        };
        let bmp = bmp_bytes(&picture).unwrap();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(&bmp[2..6], &(bmp.len() as u32).to_le_bytes());
        assert_eq!(&bmp[22..26], &(-2i32).to_le_bytes());
        assert_eq!(&bmp[54..], &[3, 2, 1, 0, 6, 5, 4, 0]);
    }
}
