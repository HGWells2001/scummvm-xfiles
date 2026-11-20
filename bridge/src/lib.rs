//! Optional, in-process ScummVM host for the pinned Shiloh core.
//! All calls run on ScummVM's main thread; audio is copied into its thread-safe queue.
//! The C++ side owns handles. No Rust allocation crosses the ABI except the opaque handle.
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
use std::panic::{catch_unwind, AssertUnwindSafe};
use xf::front::{self, Session, host, intro::Start};
use xf::hdb::Hdb;

const PIXELS: usize = 640 * 480 * 3;

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
        let prefs = self.session.as_ref().map(|s| s.host_prefs.clone());
        self.session.take();
        unsafe { if self.current != self.base { drop(Box::from_raw(self.current)); } }
        self.current = loaded.map_or(self.base, Box::into_raw);
        // SAFETY: all three Hdb allocations outlive Session. restart and Drop destroy Session first.
        let mut s = unsafe { Session::with_rules(&*self.title, &*self.current, &self.data, front::rules_hyp()) };
        s.game_cursor = true;
        s.set_saves(xf::save::Slots::open(&self.saves));
        if let Some(p) = prefs { s.host_prefs = p; }
        else if let Ok(t) = std::fs::read_to_string(self.saves.join("host-prefs.txt")) {
            if let Ok(p) = front::settings::parse_host_prefs(&t) { s.host_prefs = p; }
        }
        if self.current != self.base { s.boot(Start::Menu); s.load_game(); }
        else { s.boot(start); }
        self.mix = host::Mix { rate: 44100, volume: 1.0, ..Default::default() };
        self.session = Some(s);
    }
    fn step(&mut self, dt: u32) -> Result<(), String> {
        let s = self.session.as_mut().ok_or("Session unavailable")?;
        s.tick(dt.min(250) as u64);
        if s.prefs_saved {
            std::fs::write(self.saves.join("host-prefs.txt"), front::settings::host_prefs_text(&s.host_prefs)).map_err(|e| e.to_string())?;
            s.prefs_saved = false;
        }
        match host::request(s) {
            Some(host::HostReq::New) => self.restart(None, Start::NewGame),
            Some(host::HostReq::Quit) => self.quit = true,
            Some(host::HostReq::Load(path)) => {
                let file = path.canonicalize().map_err(|e| e.to_string())?;
                if file.parent() != Some(self.saves.as_path()) || file.extension().is_none_or(|x| !x.to_string_lossy().eq_ignore_ascii_case("x")) {
                    return Err("Save path outside this game's save directory".into());
                }
                let gam = read_database(&file)?;
                self.restart(Some(Box::new(gam)), Start::Menu);
            }
            None => {}
        }
        Ok(())
    }
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
    std::fs::read_dir(root).map_err(|e| e.to_string())?
        .filter_map(Result::ok).find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path()).ok_or_else(|| format!("Missing {name}; use the prepared data directory"))
}
fn read_database(file: &Path) -> Result<Hdb,String> {
    let size=std::fs::metadata(file).map_err(|e|e.to_string())?.len();
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
        let data = path(data)?.canonicalize().map_err(|e| e.to_string())?;
        let requested = path(saves)?;
        let parent = requested.parent().ok_or("Save directory has no parent")?.canonicalize().map_err(|e| e.to_string())?;
        let saves = parent.join(requested.file_name().ok_or("Save directory has no name")?);
        if saves.starts_with(&data) { return Err("Save directory must be outside original game data".into()); }
        std::fs::create_dir_all(&saves).map_err(|e| e.to_string())?;
        let saves = saves.canonicalize().map_err(|e| e.to_string())?;
        if saves.starts_with(&data) { return Err("Save directory must be outside original game data".into()); }
        let title = Box::new(read_database(&database(&data,"XFILES.HDB")?)?);
        let base = Box::new(read_database(&database(&data,"XFILES.GAM")?)?);
        let title = Box::into_raw(title); let base = Box::into_raw(base);
        let mut b = Box::new(Bridge { session:None,title,base,current:base,data,saves,mix:Default::default(),quit:false,error:String::new() });
        b.restart(None, match start { 1 => Start::Menu, 2 => Start::NewGame, _ => Start::Intro });
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
            1 => {host::button(s,host::Button::Left,point);},
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
        b.mix.volume=host::volume(s);b.mix.sync(&s.audio_cues());
        b.mix.fill(std::slice::from_raw_parts_mut(out,samples));Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn xfs_error(b: *const Bridge, out: *mut c_char, capacity: usize) {
    if let Some(b)=b.as_ref(){copy_text(&b.error,out,capacity);}
}
#[no_mangle]
pub unsafe extern "C" fn xfs_view(b: *const Bridge) -> u32 {
    b.as_ref().and_then(|b|b.session.as_ref()).and_then(|s|s.r.w.view).unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "C" fn xfs_cursor_shown(b: *const Bridge) -> i32 {
    catch_unwind(AssertUnwindSafe(||b.as_ref().and_then(|b|b.session.as_ref()).map_or(0,|s|i32::from(host::game_cursor_shown(s))))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
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
