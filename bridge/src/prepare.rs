//! Direct CD-folder loading and native QDMC preparation for the ScummVM GUI.
use std::path::{Path,PathBuf};
use std::process::{Command,Stdio};
use xf::media::{MediaLib,audio::{self,Pcm},pcmcache,qt::Movie};

fn child(dir:&Path,name:&str)->Option<PathBuf>{
    std::fs::read_dir(dir).ok()?.flatten().find(|e|e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).map(|e|e.path())
}

pub fn selected_root(selected:&Path)->Result<PathBuf,String>{
    let selected=selected.canonicalize().map_err(|e|e.to_string())?;
    let install=selected.file_name().is_some_and(|n|{
        let n=n.to_string_lossy();n.eq_ignore_ascii_case("MININST")||n.eq_ignore_ascii_case("MEDINST")
    });
    if install {
        if let Some(parent)=selected.parent(){
            let has_media=std::fs::read_dir(parent).ok().is_some_and(|rd|rd.flatten().any(|e|{
                let name=e.file_name();let name=name.to_string_lossy();
                name.eq_ignore_ascii_case("XV")||name.eq_ignore_ascii_case("NAV1.NMV")
            }));
            if has_media{return Ok(parent.to_path_buf());}
        }
    }
    Ok(selected)
}

pub fn data_root(selected:&Path)->Result<PathBuf,String>{
    static INIT:std::sync::OnceLock<bool>=std::sync::OnceLock::new();
    if !*INIT.get_or_init(xf::vfs::install){return Err("Game file backend unavailable".into());}
    // Installer LAYOUT.BIN is not a disc image. Build directory layers explicitly
    // instead of letting an arbitrary .BIN hide an otherwise valid copied CD.
    let mut installs=Vec::new();
    for name in ["MININST","MEDINST"]{
        if let Some(p)=child(selected,name).filter(|p|p.is_dir()){installs.push(p);}
    }
    if installs.iter().any(|p|child(p,"XFILES.HDB").is_some()){
        let mut ds=xf::vfs::DataSet::open(&installs)?;
        let root=ds.layers.iter().find_map(|l|match l{
            xf::vfs::Layer::Dir(p)=>p.parent().map(Path::to_path_buf),_=>None
        }).ok_or("CD directory layers unavailable")?;
        ds.layers.push(xf::vfs::Layer::Dir(root));
        ds.root=selected.to_path_buf();ds.sources=vec![selected.to_path_buf()];
        for (fatal,message) in ds.check(){if fatal{return Err(message);}}
        for line in ds.describe(){eprintln!("XFILES: game files from {line}");}
        return Ok(xf::vfs::mount(ds).root.clone());
    }
    xf::vfs::data_root(&[selected.to_path_buf()],&mut |line|eprintln!("XFILES: {line}"))
}

// Resolve existing ancestors before creating directories, including symlinks.
pub fn state_directory(requested:&Path,original:&Path)->Result<PathBuf,String>{
    let absolute=std::path::absolute(requested).map_err(|e|e.to_string())?;
    let mut ancestor=absolute.as_path();let mut tail=Vec::new();
    let base=loop{
        match ancestor.canonicalize(){
            Ok(p)=>break p,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
                tail.push(ancestor.file_name().ok_or("Invalid save directory")?.to_os_string());
                ancestor=ancestor.parent().ok_or("Invalid save directory")?;
            }
            Err(e)=>return Err(e.to_string()),
        }
    };
    let mut resolved=base;for p in tail.into_iter().rev(){resolved.push(p);}
    if resolved.starts_with(original){return Err("Save directory must be outside original game data".into());}
    std::fs::create_dir_all(&resolved).map_err(|e|e.to_string())?;
    let resolved=resolved.canonicalize().map_err(|e|e.to_string())?;
    if resolved.starts_with(original){return Err("Save directory must be outside original game data".into());}
    Ok(resolved)
}

fn decoder()->Result<PathBuf,String>{
    let executable=std::env::current_exe().map_err(|e|e.to_string())?;
    let name=if cfg!(windows){"xfiles-audio-decoder.exe"}else{"xfiles-audio-decoder"};
    let bundled=executable.parent().ok_or("Executable directory unavailable")?.join(name);
    if bundled.is_file(){return Ok(bundled);}
    if let Some(path)=std::env::var_os("XF_FFMPEG").filter(|p|!p.is_empty()){return Ok(path.into());}
    // For source/developer builds only. The Windows distribution includes its decoder.
    Ok(PathBuf::from("ffmpeg"))
}

pub fn audio_cache(data:&Path,saves:&Path,original:&Path)->Result<(),String>{
    let cache=state_directory(&saves.parent().ok_or("Save directory has no parent")?.join("audio-cache"),original)?;
    pcmcache::set_dir(cache.clone());
    let index=super::audio_index::Index::open(original,&cache);
    if let Some(count)=index.as_ref().and_then(|i|i.verified()){
        eprintln!("XFILES native audio: fast index, {count} cached; no movie headers probed");return Ok(());
    }
    let lib=MediaLib::new(data);
    let mut files:Vec<PathBuf>=lib.entries().map(|(_,p)|p.clone()).collect();files.sort();files.dedup();
    let mut executable=None;let mut made=0;let mut kept=0;let mut indexable=true;
    for file in files{
        let ext=file.extension().map(|x|x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(),"xmv"|"nmv"|"amv"|"dmv"|"mus"|"mov"){continue;}
        let Ok(movie)=Movie::open_header(&file)else{indexable=false;continue;};
        let Some(track)=audio::sound_track(&movie).filter(|t|&t.fourcc==b"QDMC")else{continue;};
        let bytes=xf::io::read(&file).map_err(|e|e.to_string())?;
        if pcmcache::is_fresh(&cache,&file,&bytes){kept+=1;continue;}
        let description=track.snd.ok_or("QDMC sound description missing")?;
        if executable.is_none(){executable=Some(decoder()?);}
        let tool=executable.as_ref().unwrap();
        // The core may read a virtual merged CD path. Give the decoder a seekable,
        // temporary copy outside the original data; never unpack the whole game.
        let input=cache.join(format!("qdmc-input-{}.mov",std::process::id()));
        std::fs::write(&input,&bytes).map_err(|e|e.to_string())?;
        let mut command=Command::new(tool);
        command.args(["-nostdin","-v","error","-ignore_editlist","1","-i"]).arg(&input)
            .args(["-map","0:a:0","-f","s16le","-c:a","pcm_s16le","-"]).stdin(Stdio::null());
        #[cfg(windows)]{
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let output=command.output();let _=std::fs::remove_file(&input);
        let output=output.map_err(|e|format!("Decoder audio non disponibile ({e}). Estrai il pacchetto Windows completo, incluso xfiles-audio-decoder.exe."))?;
        if !output.status.success() || output.stdout.is_empty() || output.stdout.len()%2!=0{
            return Err(format!("Decodifica audio fallita per {}: {}",file.display(),String::from_utf8_lossy(&output.stderr)));
        }
        let pcm=Pcm{rate:description.rate,channels:description.channels.max(1),
            samples:output.stdout.chunks_exact(2).map(|b|i16::from_le_bytes([b[0],b[1]])).collect()};
        pcmcache::store(&cache,&file,&bytes,&pcm,"ScummVM X-Files bundled FFmpeg 8.0 QDMC decoder")
            .map_err(|e|e.to_string())?;
        made+=1;
    }
    eprintln!("XFILES native audio: {made} prepared, {kept} cached");
    if indexable{if let Some(index)=index{index.commit(made+kept);}}
    Ok(())
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn unreadable_movie_header_never_becomes_a_verified_audio_index(){
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("xfiles-incomplete-audio-{stamp}"));
        let data=root.join("game");let saves=root.join("saves/xfiles");let cache=root.join("saves/audio-cache");
        std::fs::create_dir_all(data.join("XV")).unwrap();std::fs::create_dir_all(cache.join(pcmcache::SUBDIR)).unwrap();
        std::fs::write(data.join("XV/broken.XMV"),b"incomplete QuickTime header").unwrap();
        let selected=data.canonicalize().unwrap();
        audio_cache(&selected,&saves,&selected).unwrap();
        assert!(super::super::audio_index::Index::open(&selected,&cache).unwrap().verified().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn copied_cd_folder_is_layered_without_copying_game_files(){
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("xfiles-cd-layout-{stamp}"));
        let data=root.join("game");std::fs::create_dir_all(data.join("MININST")).unwrap();
        std::fs::create_dir_all(data.join("MEDINST")).unwrap();std::fs::create_dir_all(data.join("XV")).unwrap();
        std::fs::write(data.join("MININST/XFILES.HDB"),b"database").unwrap();
        std::fs::write(data.join("MININST/XFILES.GAM"),b"state").unwrap();
        std::fs::write(data.join("MININST/X.PFF"),b"PFF preferred").unwrap();
        std::fs::write(data.join("MEDINST/X.PFF"),b"PFF secondary").unwrap();
        std::fs::write(data.join("XV/64421.XMV"),b"movie").unwrap();
        std::fs::write(data.join("LAYOUT.BIN"),b"installer metadata, not a CD image").unwrap();
        let selected=data.canonicalize().unwrap();let merged=data_root(&selected).unwrap();
        assert_eq!(selected_root(&data.join("MININST")).unwrap(),selected);
        let lib=MediaLib::new(&merged);
        assert_eq!(xf::io::read(lib.find("xfiles.hdb").unwrap()).unwrap(),b"database");
        assert_eq!(xf::io::read(lib.find("x.pff").unwrap()).unwrap(),b"PFF preferred");
        assert_eq!(xf::io::read(lib.find("xv/64421.xmv").unwrap()).unwrap(),b"movie");
        assert!(!data.join("XFILES.HDB").exists());
        assert!(state_directory(&data.join("saves/deep"),&selected).is_err());
        assert!(!data.join("saves").exists());
        assert!(state_directory(&root.join("new/saves/game"),&selected).unwrap().is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }
}
