//! Optional, in-process ScummVM host for the pinned Shiloh core.
//! All calls run on ScummVM's main thread; audio is copied into its thread-safe queue.
//! The C++ side owns handles. No Rust allocation crosses the ABI except the opaque handle.
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
use std::panic::{catch_unwind, AssertUnwindSafe};
use xf::front::{self, Session, SCREEN_W, VIEW_X, VIEW_Y, host, intro::Start};
use xf::hdb::Hdb;
use xf::media::{FRAME_H, FRAME_W, pict::Picture};
mod prepare;
mod audio_index;
mod camera;
mod movie_hotspots;
mod photo_export;
pub fn inspect_cd_folder(selected:&Path)->Result<(),String>{
    let original=prepare::selected_root(selected)?;
    let data=prepare::data_root(&original)?;
    read_database(&database(&data,"XFILES.HDB")?)?;
    read_database(&database(&data,"XFILES.GAM")?)?;
    Ok(())
}

const PIXELS: usize = 640 * 480 * 3;
const MAX_SAVE_BYTES: usize = 128 * 1024 * 1024;
const ARREST_MOVIE: u32 = 0x4cc8;
const DISMISSAL_MOVIE: u32 = 0x4cc6;
const GUNSHOT_SOUND: u32 = 0x87b7;
const ARREST_HOTSPOT: u32 = 600;
const WAREHOUSE_WIDE: u32 = 0x5bbe;
const WAREHOUSE_BINOCULARS: u32 = 0x11432;
const BINOCULAR_BACKGROUNDS: [u32; 4] = [0x1ec6, 0x11436, 0x11438, 0x118ea];
const VIEW_RIGHT: usize = VIEW_X + FRAME_W;
const VIEW_BOTTOM: usize = VIEW_Y + FRAME_H;

fn binoculars_held(s: &Session<'_>) -> bool {
    s.r.w.dev.held == Some(1) && s.r.game_running && !s.intro_playing()
        && !s.r.game_paused() && !s.r.frozen && !s.r.rewind_box
        && s.preview.is_none() && s.alt.msgbox.is_none()
}

fn binocular_movie_position(s: &Session<'_>) -> Option<u64> {
    if !binoculars_held(s) { return None; }
    let (asset, position) = s.picture_head()?;
    (asset == WAREHOUSE_WIDE).then_some(position)
}

fn zoom_scene(frame: &mut [u8], aim: (usize, usize)) {
    let scene = frame[VIEW_Y * SCREEN_W * 3..VIEW_BOTTOM * SCREEN_W * 3].to_vec();
    let (ax, ay) = aim;
    for y in VIEW_Y..VIEW_BOTTOM {
        for x in VIEW_X..VIEW_RIGHT {
            // Keep the point under the cursor fixed as the glass moves.
            let source = (((y + ay) / 2 - VIEW_Y) * SCREEN_W + (x + ax) / 2) * 3;
            let destination = (y * SCREEN_W + x) * 3;
            frame[destination..destination + 3].copy_from_slice(&scene[source..source + 3]);
        }
    }
}

fn draw_binocular_mask(frame: &mut [u8], aim: (i32, i32), mask: &Picture) {
    let left = aim.0 - mask.width as i32 / 2;
    let top = aim.1 - mask.height as i32 / 2;
    let mask_rows = top..top + mask.height as i32;
    let clip_x = |x: i32| x.clamp(VIEW_X as i32, VIEW_RIGHT as i32) as usize;
    let visible_left = clip_x(left);
    let visible_right = clip_x(left + mask.width as i32);

    // Shiloh applies the mask only where its image overlaps the scene.
    for y in VIEW_Y..VIEW_BOTTOM {
        let row = y * SCREEN_W * 3;
        if mask_rows.contains(&(y as i32)) {
            frame[row + VIEW_X * 3..row + visible_left * 3].fill(0);
            frame[row + visible_right * 3..row + VIEW_RIGHT * 3].fill(0);
        } else {
            frame[row + VIEW_X * 3..row + VIEW_RIGHT * 3].fill(0);
        }
    }
    front::light::draw_binoc(frame, aim, mask);
}

enum MovieShot {
    UseHitTest,
    Arrest,
    Location,
    SoundOnly,
}

fn movie_shot(hit: Option<u32>, has_arrest: bool, has_targets: bool, location_ending: u32) -> MovieShot {
    use xf::world::devices::{GUN_OWNER, VIDEO_OWNER};
    match hit {
        // In a cutscene without a shot target, the view's default can refer to
        // firing into the room. Use the arrest for the shot into the cutscene.
        None | Some(GUN_OWNER | VIDEO_OWNER)
            if has_arrest || (!has_targets && location_ending == DISMISSAL_MOVIE) => MovieShot::Arrest,
        None | Some(GUN_OWNER | VIDEO_OWNER) if !has_targets && location_ending != 0 => MovieShot::Location,
        None | Some(GUN_OWNER | VIDEO_OWNER) => MovieShot::SoundOnly,
        _ => MovieShot::UseHitTest,
    }
}

fn click_frozen_note(s: &mut Session<'_>, x: i32, y: i32) -> bool {
    use xf::world::class::INTERFACE_ITEM;

    if !s.r.frozen || s.r.game_paused() || s.r.rewind_box || s.alt.msgbox.is_some() {
        return false;
    }
    let note_layouts = &s.r.w.h.exe.suicide_note_layouts;
    let mut items = s.r.clickables();
    items.retain(|item| item.cls == INTERFACE_ITEM && s.r.w.layouts().iter().any(|(layout, members)| {
        note_layouts.contains(layout) && members.iter().any(|&(owner, _)| owner == item.owner)
    }));
    let Some(item) = s.r.hit_in(&items, x, y) else { return false; };

    // Let the original button hide the layout and run its EndGame action.
    if s.r.click_clickable(item) {
        s.r.run(100_000);
        return true;
    }
    false
}

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
    movie_shot_at: Option<u64>,
    camera: camera::Camera,
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
        self.movie_shot_at = None;
        self.camera = camera::Camera::default();
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
        let mut rules = front::rules_hyp();
        // Gordon's Hauling uses a 120-second timer. Use the original NT timer range.
        rules.set_long_timers(xf::world::LongTimers::Nt);
        // SAFETY: all three Hdb allocations outlive Session. restart and Drop destroy Session first.
        let mut s = unsafe { Session::with_rules(&*self.title, &*self.current, &self.data, rules) };
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
        if self.current != self.base { s.load_game(); }
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
        let previous_exposures = s.r.w.pc.camera;
        let previous_album = photo_export::AlbumBefore::capture(s);
        s.tick(dt.min(250) as u64);
        if let Some(notice) = previous_album.and_then(|album| album.export(s, &self.saves)) {
            self.notice = Some(notice);
        }
        if s.r.w.pc.camera > previous_exposures {
            self.camera.play_shutter(&mut self.mix, camera::SHUTTER, s.now);
        }
        self.camera.update_background(s);
        let shown = s.preview.is_none().then(|| s.picture_head()).flatten();
        self.restored_hotspots.update(
            &mut s.r.movie_hotspots, shown, s.now,
            s.r.media_log.get(previous_media..).unwrap_or_default(),
        );
        if s.overlay {
            for line in s.r.log.iter().skip(previous_log) {
                if line.starts_with("H1282:") || line.starts_with("H1520:") {
                    eprintln!("XFILES hotspot: {line}");
                } else if line.contains("H182 game end") || line.starts_with("H1703:") {
                    eprintln!("XFILES game over: {line}");
                }
            }
        }
        if !s.r.game_running || s.r.rewind_box {
            self.movie_shot_at = None;
        }
        if !s.r.frozen || !s.r.game_running || s.r.rewind_box {
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
        if s.r.frozen && s.r.game_running && !s.r.rewind_box && s.r.started().contains(&GUNSHOT_SOUND) {
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
    validate_database(Hdb::open(file).map_err(|e|e.to_string())?)
}

fn validate_database(h: Hdb) -> Result<Hdb, String> {
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
pub unsafe extern "C" fn xfs_can_save(b: *const Bridge) -> i32 {
    b.as_ref().and_then(|b| b.session.as_ref()).map_or(0, |s| {
        i32::from(s.r.game_running && s.r.w.view.is_some() && !s.intro_playing()
            && !s.r.frozen && !s.r.rewind_box && !s.r.game_paused() && s.alt.msgbox.is_none()
            && s.r.video_playing().is_none() && s.r.pending() == 0)
    })
}

#[no_mangle]
pub unsafe extern "C" fn xfs_export_save(
    b: *mut Bridge,
    sink: unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize) -> i32,
    context: *mut std::ffi::c_void,
) -> i32 {
    call(b, |b| {
        if xfs_can_save(b) == 0 { return Err("Game state is not ready to save".into()); }
        let session = b.session.as_mut().ok_or("Session unavailable")?;
        let (bytes, _) = session.save_bytes()?;
        if bytes.len() > MAX_SAVE_BYTES || sink(context, bytes.as_ptr(), bytes.len()) == 0 {
            return Err("Could not export game state".into());
        }
        Ok(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn xfs_import_save(b: *mut Bridge, data: *const u8, length: usize) -> i32 {
    call(b, |b| {
        if data.is_null() || !(24..=MAX_SAVE_BYTES).contains(&length) {
            return Err("Invalid game state size".into());
        }
        let gam = validate_database(Hdb::from_bytes(std::slice::from_raw_parts(data, length).to_vec()))?;
        if xf::save::game_state_id(&gam).is_none() { return Err("Save does not contain a game state".into()); }
        b.restart(Some(Box::new(gam)), Start::Menu);
        Ok(())
    })
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
        let mut b = Box::new(Bridge { session:None,title,base,current:base,data,saves,mix:Default::default(),quit:false,error:String::new(),notice:None,arrest_movie:None,frozen_shot_at:None,movie_shot_at:None,camera:Default::default(),restored_hotspots:Default::default() });
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
                if point.is_some_and(|(x, y)| click_frozen_note(s, x, y)) {
                    return Ok(());
                }
                if let Some((x, y)) = point.filter(|&(x, y)| (20..620).contains(&x) && (90..330).contains(&y)) {
                    if s.r.w.dev.held == Some(7) && !s.intro_playing() && !s.r.game_paused()
                        && !s.r.frozen && !s.r.rewind_box && s.alt.msgbox.is_none()
                    {
                        let movie = s.r.video_playing().or(s.r.background);
                        if let Some(clip) = movie.and_then(|asset| s.clip(asset))
                            .filter(|clip| !clip.is_still() && matches!(clip.ext().as_str(), "xmv" | "mov"))
                        {
                            let hit = s.r.hit(x, y).map(|c| c.owner);
                            let hotspots = s.lib.movie_hotspots(&clip.name);
                            let has_arrest = hotspots.iter().any(|hot| hot.id == ARREST_HOTSPOT);
                            match movie_shot(hit, has_arrest, !hotspots.is_empty(), s.r.w.gun_view_video()) {
                                MovieShot::Arrest => {
                                    s.r.click(xf::world::moviehot::MOVIE_HOT_TAG | ARREST_HOTSPOT);
                                    return Ok(());
                                }
                                MovieShot::Location => {
                                    s.r.click(xf::world::devices::GUN_OWNER);
                                    return Ok(());
                                }
                                MovieShot::SoundOnly => {
                                    b.movie_shot_at = Some(s.now);
                                    b.mix.voices.retain(|voice| voice.asset != GUNSHOT_SOUND);
                                    return Ok(());
                                }
                                MovieShot::UseHitTest => {
                                    if hit.is_some_and(xf::world::moviehot::is_movie_hot) {
                                        // Prefer the scripted sound if the reaction starts one.
                                        b.movie_shot_at = Some(s.now);
                                        b.mix.voices.retain(|voice| voice.asset != GUNSHOT_SOUND);
                                    }
                                }
                            }
                        }
                    }
                }
                if s.overlay && !s.intro_playing() && !s.r.game_paused() && !s.r.frozen {
                    if let Some((x, y)) = point {
                        let movie = s.picture_head();
                        let hit = s.r.hit(x, y);
                        eprintln!("XFILES hotspot click ({x},{y}): view={:?}, movie/time={movie:?}, held={}, target={hit:?}",
                            s.r.w.view, s.r.w.inventory_value());
                    }
                }
                if !point.is_some_and(|point| b.camera.press(s, &mut b.mix, point)) {
                    let previous_exposures = s.r.w.pc.camera;
                    host::button(s,host::Button::Left,point);
                    if s.r.w.pc.camera > previous_exposures {
                        b.camera.play_shutter(&mut b.mix, camera::SHUTTER, s.now);
                    }
                }
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

fn scroll_computer_page(s: &mut Session<'_>, key: u32) -> bool {
    if !matches!(key, 0x21 | 0x22) || s.intro_playing() || s.r.game_paused()
        || s.r.frozen || s.r.rewind_box || s.alt.paused || s.alt.msgbox.is_some()
        || s.r.w.notes_ui.open
    {
        return false;
    }
    let pc = &s.r.w.pc;
    let (Some(panel), Some(max)) = (pc.panel_item, pc.scroll_max) else { return false; };
    if !s.r.w.layouts().iter().any(|(_, items)| items.iter().any(|&(item, _)| item == panel)) {
        return false;
    }
    let page = pc.scroll_page.max(1);
    let first = pc.scroll.min(max);
    s.r.w.pc.scroll = if key == 0x22 {
        first.saturating_add(page).min(max)
    } else if s.r.w.rules.h892_page_up_top && first <= pc.scroll_to_top {
        0
    } else {
        first.saturating_sub(page)
    };
    true
}

#[no_mangle]
pub unsafe extern "C" fn xfs_key(b: *mut Bridge, code: u32, modifiers: u32) -> i32 {
    call(b,|b| {
        let s = b.session.as_mut().ok_or("Session unavailable")?;
        if modifiers&1!=0 {
            if matches!(host::alt(s,code,modifiers&2!=0), front::altkeys::AltOut::Exit) {b.quit=true;}
        } else if code == 0x70 {
            s.overlay = !s.overlay;
        } else if code == 0x72 {
            let enabled = !s.r.w.closed_captioning();
            s.r.set_closed_captioning(enabled);
            b.notice = Some(if enabled { "Sottotitoli attivati" } else { "Sottotitoli disattivati" }.into());
        } else {
            // Apply queued mouse clicks first so typing reaches the newly focused field.
            if s.r.pending() != 0 {
                b.step(0)?;
                if b.quit { return Ok(()); }
            }
            let s = b.session.as_mut().ok_or("Session unavailable")?;
            if scroll_computer_page(s, code) {
                return Ok(());
            }
            if code != 0x1b && s.r.stall_until.is_some_and(|until| s.now < until)
                && !s.r.game_paused() && !s.r.rewind_box && !s.alt.paused && s.alt.msgbox.is_none()
            {
                return Ok(());
            }
            // Shiloh stores the current key separately from its queued KeyPress action.
            // Dispatch the action before ScummVM can deliver another key in the same frame.
            host::key(s, host::char_of_oem_vk(code).unwrap_or(code));
            if s.r.pending() != 0 {
                b.step(0)?;
            }
        }
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_frame(b: *mut Bridge, out: *mut u8, length: usize) -> i32 {
    if out.is_null() || length!=PIXELS {return -1;}
    call(b,|b| {
        let s = b.session.as_mut().ok_or("Session unavailable")?;
        let preview = s.preview;
        let binoculars = binoculars_held(s);
        if let Some(position) = binocular_movie_position(s) {
            if s.clip(WAREHOUSE_BINOCULARS).is_some() {
                // The wide player still owns the scene's completion and its queued actions.
                s.preview = Some((WAREHOUSE_BINOCULARS, s.picture_now().saturating_sub(position)));
            }
        }
        let authored_background = s.r.video_playing().is_none()
            && s.r.background.is_some_and(|asset| BINOCULAR_BACKGROUNDS.contains(&asset));
        let aim = (binoculars && s.preview.is_none() && !authored_background).then_some(s.mouse).flatten()
            .filter(|&(x, y)| (VIEW_X as i32..VIEW_RIGHT as i32).contains(&x)
                && (VIEW_Y as i32..VIEW_BOTTOM as i32).contains(&y));
        let held_overlay = s.r.w.dev.held_overlay;
        if binoculars {
            s.r.w.dev.held_overlay = false;
        }
        let mut frame = s.render();
        s.r.w.dev.held_overlay = held_overlay;
        s.preview = preview;
        if frame.len()!=PIXELS {return Err("Unexpected framebuffer length".into());}
        if let Some((x, y)) = aim {
            zoom_scene(&mut frame, (x as usize, y as usize));
            if let Some(mask) = s.picture(front::light::BINOCULAR_MASK) {
                draw_binocular_mask(&mut frame, (x, y), mask);
            }
        }
        std::ptr::copy_nonoverlapping(frame.as_ptr(),out,PIXELS);Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_mix(b: *mut Bridge, out: *mut i16, samples: usize) -> i32 {
    if out.is_null() || samples==0 || samples>44100 || samples%2!=0 {return -1;}
    call(b,|b| {
        let s=b.session.as_mut().ok_or("Session unavailable")?;
        let mut cues = s.audio_cues();
        if let Some(cue) = b.camera.audio(s) {
            if !cues.iter().any(|original| original.asset == cue.asset) {
                cues.push(cue);
            }
        }
        if let Some(position) = binocular_movie_position(s) {
            if let Some(cue) = reaction_cue(s, WAREHOUSE_BINOCULARS, position) {
                cues.retain(|original| original.asset != WAREHOUSE_WIDE);
                cues.push(cue);
            }
        }
        if let Some(start) = b.movie_shot_at {
            if !s.r.game_paused() && !s.r.started().contains(&GUNSHOT_SOUND)
                && b.frozen_shot_at.is_none() && !cues.iter().any(|cue| cue.asset == GUNSHOT_SOUND)
            {
                if let Some(cue) = reaction_cue(s, GUNSHOT_SOUND, s.now.saturating_sub(start)) {
                    cues.push(cue);
                } else {
                    b.movie_shot_at = None;
                }
            } else {
                b.movie_shot_at = None;
            }
        }
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
    use xf::world::devices::GUN_OWNER;

    fn with_game(test: impl FnOnce(&mut Bridge)) {
        let Ok(data) = std::env::var("XFILES_TEST_DATA") else { return; };
        let data = std::ffi::CString::new(data).unwrap();
        let saves = tempfile_path("game-over");
        let path = std::ffi::CString::new(saves.join("saves").to_str().unwrap()).unwrap();
        let mut error = [0i8; 512];
        unsafe {
            let ptr = xfs_create(data.as_ptr(), path.as_ptr(), 2, error.as_mut_ptr(), error.len());
            assert!(!ptr.is_null(), "{}", CStr::from_ptr(error.as_ptr()).to_string_lossy());
            let mut bridge = Box::from_raw(ptr);
            for _ in 0..600 {
                if !bridge.session.as_ref().unwrap().intro_playing() { break; }
                bridge.step(250).unwrap();
            }
            test(&mut bridge);
        }
        std::fs::remove_dir_all(saves).unwrap();
    }

    fn tempfile_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("xfiles-{name}-{stamp}"))
    }

    fn wait_for_game_over(bridge: &mut Bridge, ending: u32) {
        for _ in 0..720 {
            bridge.step(250).unwrap();
            let s = bridge.session.as_ref().unwrap();
            if s.r.menu_open || s.r.rewind_box { break; }
        }
        let s = bridge.session.as_ref().unwrap();
        assert!(s.r.plays.iter().any(|play| play.1 == ending), "ending {ending:#x} never played");
        assert!(s.r.menu_open || s.r.rewind_box, "ending {ending:#x} never resolved");
        assert_eq!(unsafe { xfs_can_save(bridge) }, 0);
    }

    #[test]
    fn warehouse_binoculars_show_closeup_and_restore_wide_movie() {
        use xf::world::{event, Msg, Out, RegCause};

        with_game(|bridge| {
            let s = bridge.session.as_mut().unwrap();
            s.reload_core(s.r.w.gam);
            for action in [0x7d2d, 0x1190c] {
                s.r.w.execute(Msg { action, trigger: 0 });
            }
            let tl = s.r.w.trigger_list_of(WAREHOUSE_WIDE);
            s.r.w.out.push(Out::Reg {
                owner: WAREHOUSE_WIDE, tl, cause: RegCause::PlayAsset { action: 0 },
            });
            // Register the binocular icon and use the game's own selection action.
            let icon = 0x553d;
            s.r.external_register(icon, s.r.w.trigger_list_of(icon).unwrap());
            bridge.step(100).unwrap();
            bridge.session.as_mut().unwrap().r.external_fire(WAREHOUSE_WIDE, event::ACTIVATION);
            for _ in 0..4 { bridge.step(100).unwrap(); }

            let mut wide = vec![0; PIXELS];
            let mut closeup = vec![0; PIXELS];
            unsafe { assert_eq!(xfs_frame(bridge, wide.as_mut_ptr(), PIXELS), 0); }
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.vars.get(0x421), Some(WAREHOUSE_WIDE as i32));
            assert!(s.r.click(icon));
            s.tick(0);
            assert_eq!(s.r.w.dev.held, Some(1));
            assert_eq!(binocular_movie_position(s), Some(400));
            assert!(!s.r.w.dev.held_overlay);
            assert!(reaction_cue(s, WAREHOUSE_BINOCULARS, 400).is_some());
            unsafe { assert_eq!(xfs_frame(bridge, closeup.as_mut_ptr(), PIXELS), 0); }
            let band_pixel = |x: usize, y: usize| ((y * 640 + x) * 3)..((y * 640 + x) * 3 + 3);
            assert_ne!(wide[band_pixel(320, 210)], closeup[band_pixel(320, 210)]);

            let s = bridge.session.as_mut().unwrap();
            assert!(s.r.w.held_drop());
            assert_eq!(s.r.playing(), Some(WAREHOUSE_WIDE));
            assert!(s.preview.is_none());
            unsafe { assert_eq!(xfs_frame(bridge, closeup.as_mut_ptr(), PIXELS), 0); }
            assert_eq!(wide[band_pixel(320, 210)], closeup[band_pixel(320, 210)]);
        });
    }

    #[test]
    fn binocular_zoom_keeps_the_aim_and_stays_inside_the_scene() {
        let mut frame = vec![0; PIXELS];
        for y in 0..480 {
            for x in 0..640 {
                let p = (y * 640 + x) * 3;
                frame[p..p + 3].copy_from_slice(&[x as u8, y as u8, 100]);
            }
        }
        let original = frame.clone();
        zoom_scene(&mut frame, (320, 210));
        let pixel = |x: usize, y: usize| ((y * 640 + x) * 3)..((y * 640 + x) * 3 + 3);
        assert_eq!(frame[pixel(320, 210)], original[pixel(320, 210)]);
        assert_eq!(frame[pixel(160, 150)], original[pixel(240, 180)]);
        assert_eq!(frame[pixel(160, 89)], original[pixel(160, 89)]);
        assert_eq!(frame[pixel(160, 330)], original[pixel(160, 330)]);
    }

    #[test]
    fn binocular_mask_blacks_out_uncovered_scene_edges() {
        let mask = Picture {
            width: FRAME_W,
            height: 275,
            rgb: vec![255; FRAME_W * 275 * 3],
            alpha: None,
        };
        let pixel = |x: usize, y: usize| {
            let start = (y * SCREEN_W + x) * 3;
            start..start + 3
        };
        for ((x, y), (other_x, other_y)) in [
            ((VIEW_X, VIEW_Y), (VIEW_RIGHT - 1, VIEW_BOTTOM - 1)),
            ((VIEW_RIGHT - 1, VIEW_BOTTOM - 1), (VIEW_X, VIEW_Y)),
        ] {
            let mut frame = vec![120; PIXELS];
            draw_binocular_mask(&mut frame, (x as i32, y as i32), &mask);
            assert_eq!(frame[pixel(other_x, y)], [0; 3]);
            assert_eq!(frame[pixel(x, other_y)], [0; 3]);
            assert_eq!(frame[pixel(x, y)], [120; 3]);
            assert_eq!(frame[pixel(x, VIEW_Y - 1)], [120; 3]);
            assert_eq!(frame[pixel(x, VIEW_BOTTOM)], [120; 3]);
        }
    }

    #[test]
    fn binocular_zoom_applies_to_an_ordinary_still_and_movie() {
        use xf::world::{Out, RegCause};

        let aim = (45, 105);
        with_game(|bridge| {
            let s = bridge.session.as_mut().unwrap();
            s.reload_core(s.r.w.gam);
            s.r.background = Some(0x1ec7); // Wide shot at the railway yard.
            s.mouse = Some(aim);
            s.game_cursor = false;
            bridge.step(100).unwrap();
            for movie in [None, Some(0x4d64)] {
                if let Some(owner) = movie {
                    let s = bridge.session.as_mut().unwrap();
                    let tl = s.r.w.trigger_list_of(owner);
                    s.r.w.out.push(Out::Reg { owner, tl, cause: RegCause::PlayAsset { action: 0 } });
                    s.r.external_register(0, 0);
                }
                bridge.step(100).unwrap();
                let s = bridge.session.as_mut().unwrap();
                assert_eq!(s.r.video_playing(), movie);
                s.r.w.dev.held = None;
                let plain = s.render();
                let mut expected = plain.clone();
                zoom_scene(&mut expected, (aim.0 as usize, aim.1 as usize));
                let mask = s.picture(front::light::BINOCULAR_MASK).unwrap();
                draw_binocular_mask(&mut expected, aim, mask);
                s.r.w.dev.held = Some(1);
                s.r.w.dev.held_overlay = true;
                let mut actual = vec![0; PIXELS];
                unsafe { assert_eq!(xfs_frame(bridge, actual.as_mut_ptr(), PIXELS), 0); }
                let band = 90 * 640 * 3..330 * 640 * 3;
                assert_ne!(actual[band.clone()], plain[band.clone()]);
                assert_eq!(actual[band.clone()], expected[band]);
            }
        });
    }

    #[test]
    fn computer_keeps_keyboard_input_in_order() {
        use xf::world::procs::computer::{self, MEDIA_FIELD, ING_FIELD};
        use xf::world::{class, Msg};

        with_game(|bridge| {
            let s = bridge.session.as_mut().unwrap();
            s.reload_core(s.r.w.gam);
            let media_key = s.r.w.h.exe.media_keys.iter().find_map(|&item| {
                let h = s.r.w.h;
                let list = h.read(class::INTERFACE_ITEM, item)?.u32("f44");
                let assets = h.list_of(class::ASSET_REF_LIST, list);
                let (&article, words) = assets.split_first()?;
                if article != 0xd927 { return None; } // The Coast Guard article leads to a new location.
                let word = words.iter()
                    .filter_map(|&a| computer::asset_string(&s.r.w, a))
                    .find(|word| (2..=25).contains(&word.len())
                        && word.bytes().all(|c| c.is_ascii_alphanumeric() || c == b' '))?;
                Some((article, word))
            }).expect("the original media index has no searchable article");
            s.r.viewer_show_layout(0x3b68); // Media index.
            bridge.step(16).unwrap();
            assert_eq!(bridge.session.as_ref().unwrap().r.w.dev.focus, Some(MEDIA_FIELD));
            let type_keys = |bridge: &mut Bridge, keys: &[u32]| {
                for &key in keys {
                    assert_eq!(unsafe { xfs_key(bridge, key, 0) }, 0);
                }
            };
            type_keys(bridge, &[b'A' as u32, 0xbd, b'B' as u32, 0xbe, 0xbc, 8]);
            bridge.step(16).unwrap();
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.pc.media, "A-B.");
            type_keys(bridge, &[8; 4]);
            let query = media_key.1.chars().map(host::vk_of_char).collect::<Vec<_>>();
            type_keys(bridge, &query);
            assert_eq!(bridge.session.as_ref().unwrap().r.w.pc.media, media_key.1.to_ascii_uppercase());
            type_keys(bridge, &[13]);
            for _ in 0..24 { bridge.step(250).unwrap(); }
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.pc.last_shown, media_key.0,
                "query {:?}; search executed: {}", media_key.1, s.r.execs.contains(&0xe90b));
            assert!(front::computer::asset_text(s, media_key.0).is_some_and(|text| !text.is_empty()));

            s.reload_core(s.r.w.gam);
            s.r.viewer_show_layout(0x3b04); // ING.
            bridge.step(16).unwrap();
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.dev.focus, Some(ING_FIELD));
            let plate = s.r.w.h.exe.pc.ing_plates[0];
            let reply = s.r.w.h.exe.pc.ing_plates[1];
            let query = computer::asset_string(&s.r.w, plate).unwrap();
            s.r.w.vars.set_native(0x5b2, Some(i32::from(b'L')), 0);
            s.r.w.vars.set_native(0x731, Some(i32::from(b'Z')), 0);
            let query = query.chars().map(host::vk_of_char).collect::<Vec<_>>();
            type_keys(bridge, &query);
            type_keys(bridge, &[13]);
            for _ in 0..24 { bridge.step(250).unwrap(); }
            let s = bridge.session.as_mut().unwrap();
            assert!(s.r.w.pc.texts.iter().any(|(item, src, _)| {
                *item == computer::ING_MESSAGE && *src == computer::Src::Asset(reply)
            }));

            s.reload_core(s.r.w.gam);
            s.r.viewer_show_layout(0x3b99); // Login.
            bridge.step(16).unwrap();
            let s = bridge.session.as_mut().unwrap();
            s.r.w.execute(Msg { action: 0x1020e, trigger: 0 }); // Click the login name field.
            let name = front::computer::asset_text(s, 0xe231).unwrap().to_ascii_uppercase();
            let password = front::computer::asset_text(s, 0x10d33).unwrap().to_ascii_uppercase();
            let keys = |text: &str| text.chars().map(host::vk_of_char).collect::<Vec<_>>();
            type_keys(bridge, &[8; 23]);
            type_keys(bridge, &keys(&name));
            type_keys(bridge, &[9]);
            type_keys(bridge, &keys(&password));
            assert_eq!(bridge.session.as_ref().unwrap().r.w.dev.focus, Some(0xc63c));
            type_keys(bridge, &[13]);
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.vars.get(0x9ff), Some(1));
            assert_eq!(s.r.w.vars.get(0x1352), Some(1));

            // Open the actual reader layout, with a real message from the game's inbox.
            let panel = 0xd6a6;
            s.r.w.execute(Msg { action: 0x10b56, trigger: 0 }); // SendEmail.
            s.r.w.execute(Msg { action: 0xfbef, trigger: 0 }); // Deliver pending mail.
            s.r.w.execute(Msg { action: 0xf305, trigger: 0 }); // Initialize the computer inbox.
            s.r.w.execute(Msg { action: 0xf553, trigger: 0 }); // Display the inbox.
            s.r.w.execute(Msg { action: 0xd87f, trigger: 0 }); // Select the first mail.
            s.r.w.execute(Msg { action: 0xd880, trigger: 0 }); // Mark it read.
            let selected = s.r.w.vars.get(0xa97).unwrap() as u32;
            assert!(s.r.w.px.read.contains(&selected));
            assert!(front::computer::asset_text(s, selected).is_some_and(|text| !text.is_empty()));
            s.r.viewer_show_layout(0xd63b);
            bridge.step(16).unwrap();
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.pc.last_shown, selected);
            s.render();
            assert_eq!(s.r.w.pc.panel_item, Some(panel));
            assert!(s.r.w.pc.scroll_max.is_some());
            // Exercise the page boundaries independently of the localized message's line count.
            s.r.w.pc.scroll_max = Some(10);
            s.r.w.pc.scroll_page = 6;
            s.r.w.pc.scroll_to_top = 0;
            type_keys(bridge, &[0x22, 0x22]);
            assert_eq!(bridge.session.as_ref().unwrap().r.w.pc.scroll, 10);
            type_keys(bridge, &[0x21, 0x21]);
            assert_eq!(bridge.session.as_ref().unwrap().r.w.pc.scroll, 0);
        });
    }

    #[test]
    fn camera_photographs_still_and_movie_and_uses_the_album() {
        use xf::world::{Msg, Out, RegCause};
        use xf::front::light::{CAMERA_EXPOSURES, HELD_CAMERA};

        with_game(|bridge| {
            let still = 0x1ec7;
            let movie = 0x4d64;
            let s = bridge.session.as_mut().unwrap();
            s.reload_core(s.r.w.gam);
            s.r.background = Some(still);
            bridge.step(100).unwrap();
            let s = bridge.session.as_mut().unwrap();
            s.r.w.dev.held = Some(HELD_CAMERA);
            s.r.w.dev.held_overlay = true;
            let click = |bridge: &mut Bridge, x, y| unsafe {
                assert_eq!(xfs_pointer(bridge, 0, x, y), 0);
                assert_eq!(xfs_pointer(bridge, 1, x, y), 0);
                assert_eq!(xfs_pointer(bridge, 2, x, y), 0);
            };
            click(bridge, 160, 210);
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.pc.photos, [(still, false)]);
            assert_eq!(s.r.w.pc.camera, 1);
            let still_frame = s.r.w.pc.photo_frames[0];
            assert!(s.photo_picture(still, still_frame).is_some());
            assert!(bridge.camera.audio(s).is_some_and(|cue| cue.asset == camera::SHUTTER),
                "shutter audio missing: {:?}", s.clip(camera::SHUTTER));

            let tl = s.r.w.trigger_list_of(movie);
            s.r.w.out.push(Out::Reg { owner: movie, tl, cause: RegCause::PlayAsset { action: 0 } });
            s.r.external_register(0, 0);
            bridge.step(100).unwrap();
            bridge.step(200).unwrap();
            let s = bridge.session.as_mut().unwrap();
            let (asset, position) = s.picture_head().unwrap();
            assert_eq!(asset, movie);
            assert!(position > 0);
            click(bridge, 619, 210);
            let s = bridge.session.as_mut().unwrap();
            let frame = Some(((position * 600 / 1000) as u32, [280, 0, 600, 240]));
            assert_eq!(s.r.w.pc.photos, [(still, false), (movie, false)]);
            assert_eq!(s.r.w.pc.photo_frames[1], frame);
            let photo = s.photo_picture(movie, frame).unwrap();
            assert_eq!((photo.width, photo.height), (320, 240));

            s.r.w.pc.camera = CAMERA_EXPOSURES;
            click(bridge, 320, 210);
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.pc.photos.len(), 2);
            assert_eq!(s.r.w.pc.camera, CAMERA_EXPOSURES);
            assert!(bridge.camera.audio(s).is_some_and(|cue| cue.asset == camera::EMPTY_SHUTTER),
                "empty shutter audio missing: {:?}", s.clip(camera::EMPTY_SHUTTER));
            let exports = bridge.saves.join("foto");
            assert!(!exports.exists());
            s.r.media_ended(movie);
            s.r.viewer_show_layout(0x3b80); // Original photo album.
            bridge.step(16).unwrap();
            let s = bridge.session.as_mut().unwrap();
            let [left, top, right, bottom] = s.r.clickables().into_iter()
                .find(|item| item.owner == 0xc8c5).expect("Download button").rect.map(i32::from);
            let point = ((left + right) / 2, (top + bottom) / 2);
            click(bridge, point.0, point.1);
            bridge.step(16).unwrap();
            let s = bridge.session.as_mut().unwrap();
            s.r.w.execute(Msg { action: 0xd052, trigger: 0 }); // Enlarge the first photo.
            assert_eq!(s.r.w.pc.photos, [(still, true), (movie, true)]);
            assert_eq!(s.r.w.pc.camera, 0);
            assert_eq!(s.r.w.pc.enlarged, Some(0));
            let mut files: Vec<_> = std::fs::read_dir(&exports).unwrap()
                .map(|entry| entry.unwrap().path()).collect();
            files.sort();
            assert_eq!(files.len(), 2);
            let bmp = std::fs::read(&files[1]).unwrap();
            assert_eq!(&bmp[..2], b"BM");
            assert_eq!(&bmp[18..22], &320i32.to_le_bytes());
            assert_eq!(&bmp[22..26], &(-240i32).to_le_bytes());
            assert_eq!(&bmp[54..57], &[photo.rgb[2], photo.rgb[1], photo.rgb[0]]);
            click(bridge, point.0, point.1);
            bridge.step(16).unwrap();
            assert_eq!(std::fs::read_dir(&exports).unwrap().count(), 2);
            let s = bridge.session.as_mut().unwrap();
            // The sedan's license plate is an authored evidence photo, not a generic movie crop.
            s.r.w.execute(Msg { action: 0x1372c, trigger: 0 });
            assert_eq!(s.r.w.pc.photos[2], (0x1372e, false));
            assert_eq!(s.r.w.pc.photo_frames[2], None);
            assert!(s.photo_picture(0x1372e, None).is_some());
            let (saved, _) = s.save_bytes().unwrap();
            unsafe { assert_eq!(xfs_import_save(bridge, saved.as_ptr(), saved.len()), 0); }
            let s = bridge.session.as_mut().unwrap();
            assert_eq!(s.r.w.pc.photos, [(still, true), (movie, true), (0x1372e, false)]);
            assert_eq!(s.r.w.pc.photo_frames[1], frame);
            assert_eq!(s.r.w.pc.camera, 1);
            assert_eq!(s.photo_picture(movie, frame).unwrap().rgb, photo.rgb);
            assert_eq!(std::fs::read_dir(&exports).unwrap().count(), 2);
        });
    }

    #[test]
    fn arrest_hotspot_plays_and_returns_to_menu() {
        with_game(|bridge| {
            let s = bridge.session.as_mut().unwrap();
            s.r.w.dev.held = Some(7);
            s.r.click(xf::world::moviehot::MOVIE_HOT_TAG | ARREST_HOTSPOT);
            wait_for_game_over(bridge, ARREST_MOVIE);
            let s = bridge.session.as_ref().unwrap();
            assert!(s.r.menu_open && !s.r.game_running);
            assert!(s.preview.is_none());
        });
    }

    #[test]
    fn both_frozen_notes_accept_only_their_original_button() {
        use xf::world::Out;

        with_game(|bridge| {
            for (layout, owner, action) in [(0xf57f, 0xf584, 0xf589), (0xf581, 0xf58e, 0xf593)] {
                bridge.restart(None, Start::NewGame);
                for _ in 0..600 {
                    if unsafe { xfs_can_save(bridge) } != 0 { break; }
                    bridge.step(250).unwrap();
                }
                let s = bridge.session.as_mut().unwrap();
                assert!(s.r.w.h.exe.suicide_note_layouts.contains(&layout));
                s.r.w.out.push(Out::Freeze { hotspot: 500 });
                s.r.viewer_show_layout(layout);
                bridge.step(16).unwrap();
                let s = bridge.session.as_ref().unwrap();
                assert!(s.r.frozen);
                let button = s.r.clickables().into_iter().find(|item| item.owner == owner).unwrap();
                let [left, top, right, bottom] = button.rect.map(i32::from);
                let (x, y) = ((left + right) / 2, (top + bottom) / 2);
                let (outside_x, outside_y) = [(0, 0), (639, 0), (0, 479), (639, 479)]
                    .into_iter()
                    .find(|&(x, y)| !(left..right).contains(&x) || !(top..bottom).contains(&y))
                    .unwrap_or((-1, -1));
                unsafe {
                    assert_eq!(xfs_pointer(bridge, 1, outside_x, outside_y), 0);
                    assert_eq!(xfs_pointer(bridge, 3, x, y), 0);
                    assert_eq!(xfs_tick(bridge, 16), 0);
                    let s = bridge.session.as_ref().unwrap();
                    assert!(s.r.frozen && !s.r.menu_open);
                    assert_eq!(xfs_pointer(bridge, 1, x, y), 0);
                    assert_eq!(xfs_tick(bridge, 16), 0);
                }
                let s = bridge.session.as_ref().unwrap();
                assert!(s.r.menu_open && !s.r.game_running);
                assert!(!s.r.w.layouts().iter().any(|(id, _)| *id == layout));
                assert!(s.r.log.iter().any(|line| line.starts_with(&format!("action {action:#x}: H182 game end"))));
            }
        });
    }

    #[test]
    fn original_ending_movies_dispatch_their_end_game_actions() {
        use xf::world::{event, Msg};

        const MOVIE_ENDINGS: [(u32, u32); 25] = [
            (0x4cc2, 0x9618), (0x4cc4, 0x961c), (0x5b74, 0x9b7d),
            (0x57e9, 0xa011), (0xac43, 0xacf3), (0x5b76, 0xb332),
            (0x5520, 0xb377), (0x4cc8, 0xc0f2), (0x4cd2, 0xc903),
            (0x4cc6, 0xc9b6), (0xa83e, 0xc9ba), (0x52fc, 0xdace),
            (0x52fa, 0xdacf), (0x54dc, 0xdad3), (0x54de, 0xdad6),
            (0x54da, 0xdad9), (0x5442, 0x1047f), (0xa162, 0x10bc7),
            (0xa164, 0x10bcd), (0xcafd, 0x10bd3), (0xcaff, 0x10bd9),
            (0xcb01, 0x10be3), (0x52b0, 0x15e6d), (0x5314, 0x15fc4),
            (0xbf7b, 0x16b30),
        ];

        with_game(|bridge| {
            let s = bridge.session.as_mut().unwrap();
            for (movie, action) in MOVIE_ENDINGS {
                s.reload_core(s.r.w.gam);
                assert!(s.clip(movie).is_some(), "missing ending movie {movie:#x}");
                // Exercise the direct ending after the one-time alternate has played.
                for getter in [0xf498, 0xf49a] {
                    s.r.w.execute(Msg { action: getter, trigger: 0 });
                }
                s.r.w.vars.set_native(0x5d7, Some(b'd' as i32), 0);
                let triggers = s.r.w.trigger_list_of(movie).unwrap();
                s.r.external_register(movie, triggers);
                s.r.external_fire(movie, event::DEACTIVATION);
                for _ in 0..480 {
                    s.tick(250);
                    if s.r.menu_open || s.r.rewind_box { break; }
                }
                let end_log = format!("action {action:#x}: H182 game end");
                assert!(s.r.log.iter().any(|line| line.starts_with(&end_log)),
                    "movie {movie:#x} did not dispatch {action:#x}: {:?}", s.r.log);
                assert!(s.r.rewind_box || (s.r.menu_open && !s.r.game_running));
            }

            // This view ends the game on a timer instead of a movie's deactivation.
            const TIMED_VIEW: u32 = 0x16f08;
            s.reload_core(s.r.w.gam);
            s.r.w.view = Some(TIMED_VIEW);
            s.r.external_register(TIMED_VIEW, s.r.w.trigger_list_of(TIMED_VIEW).unwrap());
            s.tick(0);
            let timer = s.r.timers.iter().find(|timer| timer.id == 10).unwrap();
            assert_eq!(timer.due - s.now, 14_000);
            s.tick(13_999);
            assert!(!s.r.menu_open && !s.r.rewind_box);
            s.tick(1);
            assert!(s.r.log.iter().any(|line| line.starts_with("action 0x16f16: H182 game end")));
            assert!(s.r.rewind_box || (s.r.menu_open && !s.r.game_running));
        });
    }

    #[test]
    fn shot_in_ordinary_office_movie_plays_arrest() {
        with_game(|bridge| {
            let s = bridge.session.as_mut().unwrap();
            s.reload_core(s.r.w.gam);
            s.r.w.location = Some(0x12f);
            s.r.w.dev.held = Some(7);
            assert_eq!(s.r.w.gun_view_video(), DISMISSAL_MOVIE);
            let movie = 0x4d64;
            let clip = s.clip(movie).unwrap();
            assert!(clip.ext() == "xmv" && !clip.is_still());
            assert!(s.lib.movie_hotspots(&clip.name).is_empty());
            // Exercise an ordinary cutscene with the field-office location and no scripted target.
            s.r.background = Some(movie);
            unsafe { assert_eq!(xfs_pointer(bridge, 1, 320, 200), 0); }
            wait_for_game_over(bridge, ARREST_MOVIE);
            let s = bridge.session.as_ref().unwrap();
            assert!(!s.r.plays.iter().any(|play| play.1 == DISMISSAL_MOVIE));

            let s = bridge.session.as_mut().unwrap();
            s.reload_core(s.r.w.gam);
            s.r.w.location = Some(0x12f);
            s.r.w.dev.held = Some(7);
            assert_eq!(s.r.hit(320, 200).map(|target| target.owner), Some(GUN_OWNER));
            unsafe { assert_eq!(xfs_pointer(bridge, 1, 320, 200), 0); }
            wait_for_game_over(bridge, DISMISSAL_MOVIE);
        });
    }

    #[test]
    fn manifest_timer_runs_to_game_over() {
        with_game(|bridge| {
            use xf::world::event;
            let s = bridge.session.as_mut().unwrap();
            // Reproduce the manifest player's deactivation with Gordon's Hauling active.
            s.r.w.view = Some(0x1b41);
            s.r.w.location = Some(0x1ac2);
            s.r.external_register(0x1ac2, 0x6387);
            s.r.external_register(0x527e, 0x9d01);
            s.r.external_fire(0x527e, event::DEACTIVATION);
            let mut armed = None;
            for _ in 0..120 {
                bridge.step(250).unwrap();
                let s = bridge.session.as_ref().unwrap();
                if let Some(timer) = s.r.timers.iter().find(|timer| timer.id == 7) {
                    assert_eq!(timer.paused, None);
                    armed = Some(s.now);
                    break;
                }
            }
            let armed = armed.expect("the manifest must arm timer 7");
            for _ in 0..960 {
                bridge.step(250).unwrap();
                let s = bridge.session.as_ref().unwrap();
                if s.r.menu_open || s.r.rewind_box {
                    assert!(s.now - armed >= 119_750);
                    assert!(s.r.posts.iter().any(|message| message.action == 0xa086));
                    assert!(s.r.log.iter().any(|line| line.contains("H182 game end")));
                    assert!(s.r.rewind_box, "this route offers a retry");
                    unsafe {
                        assert_eq!(xfs_can_save(bridge), 0);
                        assert_eq!(xfs_pointer(bridge, 1, 320, 278), 0);
                        assert!(bridge.session.as_ref().unwrap().r.rewind_box);
                        assert_eq!(xfs_pointer(bridge, 2, 320, 278), 0);
                        assert_eq!(xfs_pointer(bridge, 1, 365, 278), 0);
                        assert_eq!(xfs_pointer(bridge, 2, 365, 278), 0);
                    }
                    let s = bridge.session.as_ref().unwrap();
                    assert!(s.r.menu_open && !s.r.game_running && !s.r.rewind_box);
                    return;
                }
            }
            let s = bridge.session.as_ref().unwrap();
            panic!("timer 7 never ran the bomb's EndGame action: {:?}",
                s.r.log.iter().rev().take(15).collect::<Vec<_>>());
        });
    }
    #[test]
    fn scummvm_save_round_trip() {
        let Ok(data) = std::env::var("XFILES_TEST_DATA") else { return; };
        let data = std::ffi::CString::new(data).unwrap();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let saves = std::env::temp_dir().join(format!("xfiles-native-save-{stamp}")).join("saves");
        let save_path = std::ffi::CString::new(saves.to_str().unwrap()).unwrap();
        let mut error = [0i8; 512];
        unsafe extern "C" fn collect(context: *mut std::ffi::c_void, data: *const u8, size: usize) -> i32 {
            let bytes = &mut *context.cast::<Vec<u8>>();
            bytes.extend_from_slice(std::slice::from_raw_parts(data, size));
            1
        }
        unsafe {
            let core = xfs_create(data.as_ptr(), save_path.as_ptr(), 2, error.as_mut_ptr(), error.len());
            assert!(!core.is_null(), "{}", CStr::from_ptr(error.as_ptr()).to_string_lossy());
            for _ in 0..2400 {
                assert_eq!(xfs_tick(core, 50), 0);
                if xfs_can_save(core) != 0 { break; }
            }
            assert_eq!(xfs_can_save(core), 1);
            let view = xfs_view(core);
            let mut payload = Vec::<u8>::new();
            assert_eq!(xfs_export_save(core, collect, (&mut payload as *mut Vec<u8>).cast()), 0);
            assert_eq!(xfs_import_save(core, payload.as_ptr(), 12), -1);
            assert_eq!(xfs_view(core), view);
            assert_eq!(xfs_import_save(core, payload.as_ptr(), payload.len()), 0);
            for _ in 0..120 { assert_eq!(xfs_tick(core, 16), 0); }
            assert_eq!(xfs_view(core), view);
            assert!(!(*core).session.as_ref().unwrap().r.menu_open);
            xfs_destroy(core);
        }
    }
    #[test]
    fn movie_shots_keep_scripted_targets_and_allow_shots_elsewhere() {
        use xf::world::{devices::VIDEO_OWNER, moviehot::MOVIE_HOT_TAG};
        assert!(matches!(movie_shot(Some(MOVIE_HOT_TAG | 221), true, true, 0x4cc6), MovieShot::UseHitTest));
        assert!(matches!(movie_shot(Some(VIDEO_OWNER), true, true, 0x4cc8), MovieShot::Arrest));
        assert!(matches!(movie_shot(Some(GUN_OWNER), false, false, DISMISSAL_MOVIE), MovieShot::Arrest));
        assert!(matches!(movie_shot(Some(VIDEO_OWNER), false, false, DISMISSAL_MOVIE), MovieShot::Arrest));
        assert!(matches!(movie_shot(Some(VIDEO_OWNER), false, false, 0xa83e), MovieShot::Location));
        assert!(matches!(movie_shot(Some(GUN_OWNER), false, true, 0x4cc6), MovieShot::SoundOnly));
        assert!(matches!(movie_shot(None, false, false, 0), MovieShot::SoundOnly));
    }
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
