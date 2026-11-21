//! Optional, in-process ScummVM host for the pinned Shiloh core.
//! All calls run on ScummVM's main thread; audio is copied into its thread-safe queue.
//! The C++ side owns handles. No Rust allocation crosses the ABI except the opaque handle.
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
use std::panic::{catch_unwind, AssertUnwindSafe};
use xf::front::{self, Session, host, intro::Start};
use xf::hdb::Hdb;
mod prepare;
mod audio_index;
mod movie_hotspots;
pub fn inspect_cd_folder(selected:&Path)->Result<(),String>{
    let original=prepare::selected_root(selected)?;
    let data=prepare::data_root(&original)?;
    read_database(&database(&data,"XFILES.HDB")?)?;
    read_database(&database(&data,"XFILES.GAM")?)?;
    Ok(())
}

const PIXELS: usize = 640 * 480 * 3;
const ARREST_MOVIE: u32 = 0x4cc8;
const GUNSHOT_SOUND: u32 = 0x87b7;

// A successful hotspot 600 hit freezes the world before posting this movie.
// Its picture and sound need a clock independent of the frozen scene.
struct ArrestMovie {
    started_at: u64,
    duration: u64,
    finished: bool,
}

fn reaction_cue(s: &mut Session<'_>, asset: u32, elapsed: u64) -> Option<front::Cue> {
    let clip = s.clip(asset)?;
    let pcm = s.sound(asset)?;
    let frame = |ms: u64| (ms * u64::from(pcm.rate) / 1000) as usize;
    let total = pcm.frames();
    let start = frame(u64::from(clip.start)).min(total);
    let end = if clip.end > clip.start { frame(u64::from(clip.end)).min(total) } else { total };
    let pos = start.saturating_add(frame(elapsed));
    (pos < end).then_some(front::Cue { asset, pcm, pos, start, end, looped: false })
}

pub struct Bridge {
    // These boxes have stable addresses. Session is always dropped before any referenced box.
    session: Option<Session<'static>>,
    title: *mut Hdb,
    base: *mut Hdb,
    current: *mut Hdb,
    data: PathBuf,
    saves: PathBuf,
    mix: host::Mix,
    quit: bool,
    error: String,
    notice: Option<String>,
    arrest_movie: Option<ArrestMovie>,
    frozen_shot_at: Option<u64>,
    restored_hotspots: movie_hotspots::RestoredHotspots,
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.session.take();
        unsafe {
            if self.current != self.base { drop(Box::from_raw(self.current)); }
            drop(Box::from_raw(self.base));
            drop(Box::from_raw(self.title));
        }
    }
}

impl Bridge {
    fn restart(&mut self, loaded: Option<Box<Hdb>>, start: Start) {
        self.arrest_movie = None;
        self.frozen_shot_at = None;
        self.restored_hotspots = movie_hotspots::RestoredHotspots::default();
        let overlay = self.session.as_ref().is_some_and(|s| s.overlay);
        let captions = self.session.as_ref().map(|s| s.r.w.closed_captioning());
        let prefs = self.session.as_ref().map(|s| s.host_prefs.clone());
        let random_seed = self.session.as_ref().and_then(|s| s.r.w.live_rand).unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u32
        });
        self.session.take();
        unsafe { if self.current != self.base { drop(Box::from_raw(self.current)); } }
        self.current = loaded.map_or(self.base, Box::into_raw);
        // SAFETY: all three Hdb allocations outlive Session. restart and Drop destroy Session first.
        let mut s = unsafe { Session::with_rules(&*self.title, &*self.current, &self.data, front::rules_hyp()) };
        // Match Shiloh's interactive host. Timed encounters need live random draws
        // to select their target videos and resolve their outcomes.
        s.r.w.live_rand = Some(random_seed);
        s.game_cursor = true;
        s.overlay = overlay;
        s.set_saves(xf::save::Slots::open(&self.saves));
        if let Some(p) = prefs { s.host_prefs = p; }
        else if let Ok(t) = std::fs::read_to_string(self.saves.join("host-prefs.txt")) {
            if let Ok(p) = front::settings::parse_host_prefs(&t) { s.host_prefs = p; }
        }
        if self.current != self.base { s.boot(Start::Menu); s.load_game(); }
        else { s.boot(start); }
        if let Some(captions) = captions {
            s.r.set_closed_captioning(captions);
        }
        self.mix = host::Mix { rate: 44100, volume: 1.0, ..Default::default() };
        self.session = Some(s);
    }
    fn load_slot(&mut self, path: &Path) -> Result<(), String> {
        let gam = read_save_slot(&self.saves, path)?;
        self.restart(Some(Box::new(gam)), Start::Menu);
        Ok(())
    }

    fn step(&mut self, dt: u32) -> Result<(), String> {
        let s = self.session.as_mut().ok_or("Session unavailable")?;
        let previous_plays = s.r.plays.len();
        let previous_log = s.r.log.len();
        let previous_media = s.r.media_log.len();
        s.tick(dt.min(250) as u64);
        let shown = s.preview.is_none().then(|| s.picture_head()).flatten();
        self.restored_hotspots.update(
            &mut s.r.movie_hotspots, shown, s.now,
            s.r.media_log.get(previous_media..).unwrap_or_default(),
        );
        if s.overlay {
            for line in s.r.log.iter().skip(previous_log) {
                if line.starts_with("H1282:") || line.starts_with("H1520:") {
                    eprintln!("XFILES hotspot: {line}");
                }
            }
        }
        if !s.r.frozen {
            self.frozen_shot_at = None;
            if self.arrest_movie.take().is_some() {
                s.preview = None;
            }
        } else if let Some(movie) = self.arrest_movie.as_mut() {
            if !movie.finished && s.now.saturating_sub(movie.started_at) >= movie.duration {
                movie.finished = true;
                s.r.media_ended(ARREST_MOVIE);
            }
            if movie.finished {
                // Hold the last frame while the world remains frozen.
                s.preview = Some((ARREST_MOVIE, s.now.saturating_sub(movie.duration - 1)));
            }
        } else if s.r.plays.iter().skip(previous_plays).any(|play| play.1 == ARREST_MOVIE) {
            let duration = s.duration(ARREST_MOVIE).max(1);
            s.play_preview(ARREST_MOVIE);
            self.arrest_movie = Some(ArrestMovie { started_at: s.now, duration, finished: false });
        }
        if s.r.frozen && s.r.started().contains(&GUNSHOT_SOUND) {
            let started_at = *self.frozen_shot_at.get_or_insert(s.now);
            if s.now.saturating_sub(started_at) >= s.duration(GUNSHOT_SOUND) {
                s.r.media_ended(GUNSHOT_SOUND);
                self.frozen_shot_at = None;
            }
        }
        if s.prefs_saved {
            if let Err(error) = std::fs::write(self.saves.join("host-prefs.txt"), front::settings::host_prefs_text(&s.host_prefs)) {
                eprintln!("XFILES: could not write preferences: {error}");
                self.notice = Some("Impossibile salvare le preferenze.".into());
            }
            s.prefs_saved = false;
        }
        match host::request(s) {
            Some(host::HostReq::New) => self.restart(None, Start::NewGame),
            Some(host::HostReq::Quit) => self.quit = true,
            Some(host::HostReq::Load(path)) => {
                if let Err(error) = self.load_slot(&path) {
                    eprintln!("XFILES: could not load save: {error}");
                    self.notice = Some("Impossibile caricare il salvataggio.".into());
                }
            }
            None => {}
        }
        Ok(())
    }
}

fn read_save_slot(saves: &Path, path: &Path) -> Result<Hdb, String> {
    let file = path.canonicalize().map_err(|e| e.to_string())?;
    if file.parent() != Some(saves) || !file.extension().is_some_and(|x| x.eq_ignore_ascii_case("x")) {
        return Err("Save path outside this game's save directory or invalid extension".into());
    }
    let gam = read_database(&file)?;
    if xf::save::game_state_id(&gam).is_none() {
        return Err("Save does not contain a game state".into());
    }
    Ok(gam)
}

unsafe fn path(p: *const c_char) -> Result<PathBuf, String> {
    if p.is_null() { return Err("Missing path".into()); }
    let s = CStr::from_ptr(p).to_str().map_err(|_| "Path is not UTF-8")?;
    if s.is_empty() { return Err("Empty path".into()); }
    Ok(PathBuf::from(s))
}
unsafe fn copy_text(text: &str, out: *mut c_char, capacity: usize) {
    if out.is_null() || capacity == 0 { return; }
    let n = text.len().min(capacity - 1);
    std::ptr::copy_nonoverlapping(text.as_ptr(), out as *mut u8, n);
    *out.add(n) = 0;
}
fn database(root: &Path, name: &str) -> Result<PathBuf, String> {
    xf::io::read_dir(root).map_err(|e| e.to_string())?.into_iter()
        .find(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name))
        .map(|e| e.path).ok_or_else(|| format!("Missing {name} in the selected game directory"))
}
fn read_database(file: &Path) -> Result<Hdb,String> {
    prepare::ensure_file_backend()?;
    let size=xf::io::fs().size(file).map_err(|e|e.to_string())?;
    if !(24..=128*1024*1024).contains(&size) {return Err("Invalid database size".into());}
    let h=Hdb::open(file).map_err(|e|e.to_string())?;
    if h.version!=0x501 || !(1..=5).contains(&h.app_version) || h.nclasses==0 || h.nclasses>4096 || h.classes.len()!=h.nclasses as usize {
        return Err("Unsupported or inconsistent NeoAccess database".into());
    }
    for c in &h.classes {
        if c.objects.len()!=c.count as usize {return Err("Database object count mismatch".into());}
        for &(_,mark) in &c.objects {
            if mark==0 || mark as usize+6>h.d.len() {return Err("Database record outside file".into());}
        }
    }
    Ok(h)
}

#[no_mangle]
pub unsafe extern "C" fn xfs_create(data: *const c_char, saves: *const c_char, start: u32, error: *mut c_char, capacity: usize) -> *mut Bridge {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Box<Bridge>, String> {
        let started=std::time::Instant::now();
        let original = prepare::selected_root(&path(data)?)?;
        let requested = path(saves)?;
        let saves = prepare::state_directory(&requested,&original)?;
        let data = prepare::data_root(&original)?;
        let title = Box::new(read_database(&database(&data,"XFILES.HDB")?)?);
        let base = Box::new(read_database(&database(&data,"XFILES.GAM")?)?);
        let media=xf::media::MediaLib::new(&data);
        let pictures=media.find("x.pff").ok_or("Manca X.PFF: copia anche i file delle cartelle MININST e MEDINST dei CD.")?;
        if xf::io::read_range(pictures,0,4).map_err(|e|e.to_string())?.as_slice()!=b"PFF " {
            return Err("X.PFF non contiene i dati completi del gioco. Ricopia i file dati dei CD nella cartella selezionata.".into());
        }
        let audio_started=std::time::Instant::now();
        prepare::audio_cache(&data,&saves,&original)?;
        let audio_ms=audio_started.elapsed().as_millis();
        let title = Box::into_raw(title); let base = Box::into_raw(base);
        let mut b = Box::new(Bridge { session:None,title,base,current:base,data,saves,mix:Default::default(),quit:false,error:String::new(),notice:None,arrest_movie:None,frozen_shot_at:None,restored_hotspots:Default::default() });
        b.restart(None, match start { 1 => Start::Menu, 2 => Start::NewGame, _ => Start::Intro });
        eprintln!("XFILES startup: audio={audio_ms}ms, total={}ms",started.elapsed().as_millis());
        Ok(b)
    }));
    match result {
        Ok(Ok(b)) => Box::into_raw(b),
        Ok(Err(e)) => { copy_text(&e,error,capacity); std::ptr::null_mut() },
        Err(_) => { copy_text("Shiloh initialization panic",error,capacity); std::ptr::null_mut() },
    }
}
#[no_mangle]
pub unsafe extern "C" fn xfs_destroy(b: *mut Bridge) { if !b.is_null() { drop(Box::from_raw(b)); } }

unsafe fn call(b: *mut Bridge, f: impl FnOnce(&mut Bridge) -> Result<(),String>) -> i32 {
    let Some(b) = b.as_mut() else { return -1; };
    match catch_unwind(AssertUnwindSafe(||f(b))) {
        Ok(Ok(())) => if b.quit {1} else {0},
        Ok(Err(e)) => {b.error=e;-1},
        Err(_) => {b.error="Shiloh runtime panic".into();-1},
    }
}
#[no_mangle]
pub unsafe extern "C" fn xfs_tick(b: *mut Bridge, dt: u32) -> i32 { call(b,|b| b.step(dt)) }
#[no_mangle]
pub unsafe extern "C" fn xfs_pointer(b: *mut Bridge, kind: u32, x: i32, y: i32) -> i32 {
    call(b,|b| {
        let s=b.session.as_mut().ok_or("Session unavailable")?;
        let point=(x>=0 && x<640 && y>=0 && y<480).then_some((x,y));
        match kind {
            0 => {host::motion(s,point);},
            1 => {
                if s.overlay && !s.intro_playing() && !s.r.game_paused() && !s.r.frozen {
                    if let Some((x, y)) = point {
                        let movie = s.picture_head();
                        let hit = s.r.hit(x, y);
                        eprintln!("XFILES hotspot click ({x},{y}): view={:?}, movie/time={movie:?}, held={}, target={hit:?}",
                            s.r.w.view, s.r.w.inventory_value());
                    }
                }
                host::button(s,host::Button::Left,point);
            },
            2 => {host::release(s,host::Button::Left,point);},
            3 => {host::button(s,host::Button::Right,point);},
            4 => {host::release(s,host::Button::Right,point);},
            5 | 6 => {s.wheel(x,y,if kind==5 {-3} else {3});},
            _ => return Err("Unknown pointer input".into()),
        }
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_key(b: *mut Bridge, code: u32, modifiers: u32) -> i32 {
    call(b,|b| {
        let s=b.session.as_mut().ok_or("Session unavailable")?;
        if modifiers&1!=0 {
            if matches!(host::alt(s,code,modifiers&2!=0), front::altkeys::AltOut::Exit) {b.quit=true;}
        } else if code == 0x70 {
            s.overlay = !s.overlay;
        } else if code == 0x72 {
            let enabled = !s.r.w.closed_captioning();
            s.r.set_closed_captioning(enabled);
            b.notice = Some(if enabled { "Sottotitoli attivati" } else { "Sottotitoli disattivati" }.into());
        } else {host::key(s,code);}
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_frame(b: *mut Bridge, out: *mut u8, length: usize) -> i32 {
    if out.is_null() || length!=PIXELS {return -1;}
    call(b,|b| {
        let frame=b.session.as_mut().ok_or("Session unavailable")?.render();
        if frame.len()!=PIXELS {return Err("Unexpected framebuffer length".into());}
        std::ptr::copy_nonoverlapping(frame.as_ptr(),out,PIXELS);Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_mix(b: *mut Bridge, out: *mut i16, samples: usize) -> i32 {
    if out.is_null() || samples==0 || samples>44100 || samples%2!=0 {return -1;}
    call(b,|b| {
        let s=b.session.as_mut().ok_or("Session unavailable")?;
        let mut cues = s.audio_cues();
        if s.r.frozen && !s.r.menu_open && !s.r.menu_screen_up() {
            if let Some(start) = b.frozen_shot_at {
                cues.extend(reaction_cue(s, GUNSHOT_SOUND, s.now.saturating_sub(start)));
            }
            if let Some(movie) = b.arrest_movie.as_ref().filter(|m| !m.finished) {
                cues.extend(reaction_cue(s, ARREST_MOVIE, s.now.saturating_sub(movie.started_at)));
            }
        }
        b.mix.volume=host::volume(s);b.mix.sync(&cues);
        b.mix.fill(std::slice::from_raw_parts_mut(out,samples));Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_error(b: *const Bridge, out: *mut c_char, capacity: usize) {
    if let Some(b)=b.as_ref(){copy_text(&b.error,out,capacity);}
}
#[no_mangle]
pub unsafe extern "C" fn xfs_take_notice(b: *mut Bridge, out: *mut c_char, capacity: usize) -> i32 {
    if out.is_null() || capacity == 0 { return 0; }
    let Some(notice) = b.as_mut().and_then(|b| b.notice.take()) else { return 0; };
    copy_text(&notice, out, capacity);
    1
}
#[no_mangle]
pub unsafe extern "C" fn xfs_view(b: *const Bridge) -> u32 {
    b.as_ref().and_then(|b|b.session.as_ref()).and_then(|s|s.r.w.view).unwrap_or(0)
}
/// Diagnostic for the cold-start regression check, including the menu's fade-in.
pub unsafe fn menu_ready(b:*const Bridge)->bool{
    b.as_ref().and_then(|b|b.session.as_ref()).is_some_and(|s|
        !s.intro_playing() && s.r.menu_open && s.menu.shown && !s.menu.opening())
}
#[no_mangle]
pub unsafe extern "C" fn xfs_cursor_shown(b: *const Bridge) -> i32 {
    catch_unwind(AssertUnwindSafe(||b.as_ref().and_then(|b|b.session.as_ref()).map_or(0,|s|i32::from(host::game_cursor_shown(s))))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_and_truncated_save_slots() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let saves = std::env::temp_dir().join(format!("xfiles-bad-save-{stamp}"));
        std::fs::create_dir(&saves).unwrap();
        let saves = saves.canonicalize().unwrap();
        let slot = saves.join("TEST.x");
        assert!(read_save_slot(&saves, &slot).is_err());
        std::fs::write(&slot, b"partial save").unwrap();
        assert!(matches!(read_save_slot(&saves, &slot), Err(error) if error == "Invalid database size"));
        assert_eq!(std::fs::read(&slot).unwrap(), b"partial save");
        std::fs::remove_file(slot).unwrap();
        std::fs::remove_dir(saves).unwrap();
    }
    #[test] fn null_handles_and_buffers_are_rejected() { unsafe {
        assert_eq!(xfs_tick(std::ptr::null_mut(),16),-1);
        assert_eq!(xfs_frame(std::ptr::null_mut(),std::ptr::null_mut(),PIXELS),-1);
        assert_eq!(xfs_mix(std::ptr::null_mut(),std::ptr::null_mut(),1470),-1);
        assert_eq!(xfs_pointer(std::ptr::null_mut(),0,10,10),-1);
        assert_eq!(xfs_key(std::ptr::null_mut(),13,0),-1);
        xfs_destroy(std::ptr::null_mut());
    }}
    #[test] fn initialization_errors_are_bounded_and_terminated() { unsafe {
        let mut err=[42i8;8];
        let p=xfs_create(std::ptr::null(),std::ptr::null(),0,err.as_mut_ptr(),err.len());
        assert!(p.is_null());assert_eq!(err[7],0);
    }}
    #[test] fn corrupt_database_is_rejected_before_session_creation() { unsafe {
        let unique=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("xfiles-bridge-test-{unique}"));
        let data=root.join("data");let saves=root.join("saves");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("XFILES.HDB"),[0u8;24]).unwrap();
        std::fs::write(data.join("XFILES.GAM"),[0u8;24]).unwrap();
        let data=std::ffi::CString::new(data.to_str().unwrap()).unwrap();
        let saves=std::ffi::CString::new(saves.to_str().unwrap()).unwrap();
        let mut err=[0i8;256];
        let h=xfs_create(data.as_ptr(),saves.as_ptr(),0,err.as_mut_ptr(),err.len());
        assert!(h.is_null());
        assert!(CStr::from_ptr(err.as_ptr()).to_string_lossy().contains("database"));
    }}
}
