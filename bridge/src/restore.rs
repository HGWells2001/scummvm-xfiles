//! Restore mistakenly copied Macintosh empty resource forks from the owner's CDs.
//! After restoration all game reads remain in the originally selected directory.
use std::path::{Path,PathBuf};
use std::io::{Read,Seek,SeekFrom,Write};
#[cfg(test)]const HEADER:[u8;16]=[0,0,1,0,0,0,1,0,0,0,0,0,0,0,0,30];
const BACKUP:&str=".xfiles-resourcefork-backup";

pub fn child(dir:&Path,name:&str)->Option<PathBuf>{
    std::fs::read_dir(dir).ok()?.flatten().find(|e|e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).map(|e|e.path())
}
fn fork_map(header:&[u8],size:u64)->Option<u64>{
    if header.len()<16{return None;}
    let word=|n|u32::from_be_bytes(header[n..n+4].try_into().unwrap()) as u64;
    let (data,map,len,maplen)=(word(0),word(4),word(8),word(12));
    // A Macintosh resource fork, including nonempty QuickTime metadata without
    // its separate data fork. Require bounded sections and the mirrored header.
    (data==256 && map==data+len && maplen>=30 && map.checked_add(maplen)==Some(size)).then_some(map)
}
fn resource_fork(bytes:&[u8])->bool{
    let Some(map)=fork_map(bytes,bytes.len() as u64)else{return false;};
    bytes.get(map as usize..map as usize+16)==bytes.get(..16)
}
fn file_resource_fork(path:&Path,size:u64)->Result<bool,String>{
    if size<286{return Ok(false);}
    let mut file=std::fs::File::open(path).map_err(|e|e.to_string())?;
    let mut header=[0;16];file.read_exact(&mut header).map_err(|e|e.to_string())?;
    let Some(map)=fork_map(&header,size)else{return Ok(false);};
    file.seek(SeekFrom::Start(map)).map_err(|e|e.to_string())?;
    let mut mirror=[0;16];file.read_exact(&mut mirror).map_err(|e|e.to_string())?;
    Ok(header==mirror)
}
fn candidates(root:&Path)->Result<Vec<(PathBuf,PathBuf)>,String>{
    let mut out=Vec::new();
    let mut dirs=vec![root.to_path_buf()];
    for name in ["MININST","MEDINST","XG","XN","XS","XV","XT"]{
        if let Some(p)=child(root,name){
            let kind=std::fs::symlink_metadata(&p).map_err(|e|e.to_string())?;
            if kind.is_dir() && !kind.file_type().is_symlink(){
                if name=="MININST" || name=="MEDINST"{
                    for media in ["XG","XN","XS","XV","XT"]{
                        if let Some(sub)=child(&p,media){
                            let kind=std::fs::symlink_metadata(&sub).map_err(|e|e.to_string())?;
                            if kind.is_dir() && !kind.file_type().is_symlink(){dirs.push(sub);}
                        }
                    }
                }
                dirs.push(p);
            }
        }
    }
    for dir in dirs{
        for entry in std::fs::read_dir(&dir).map_err(|e|e.to_string())?{
            let e=entry.map_err(|e|e.to_string())?;
            if !e.file_type().map_err(|e|e.to_string())?.is_file(){continue;}
            let path=e.path();
            let ext=path.extension().map(|x|x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if !matches!(ext.as_str(),"hot"|"nmv"|"pff"|"xmv"|"xtx"|"amv"|"dmv"|"mus"){continue;}
            if !file_resource_fork(&path,e.metadata().map_err(|e|e.to_string())?.len())?{continue;}
            let relative=path.strip_prefix(root).map_err(|e|e.to_string())?.to_path_buf();
            let mut parts=relative.components();
            let install=parts.next().is_some_and(|n|{
                let n=n.as_os_str().to_string_lossy();n.eq_ignore_ascii_case("MININST")||n.eq_ignore_ascii_case("MEDINST")
            });
            let key=if install{parts.as_path().to_path_buf()}else{relative.clone()};
            out.push((relative,key));
        }
    }
    out.sort();Ok(out)
}
fn database(root:&Path,name:&str)->Result<Vec<u8>,String>{
    for folder in ["MININST","MEDINST",""]{
        let dir=if folder.is_empty(){Some(root.to_path_buf())}else{child(root,folder)};
        if let Some(path)=dir.and_then(|p|child(&p,name)){
            return std::fs::read(path).map_err(|e|e.to_string());
        }
    }
    Err(format!("{name} non trovato nella cartella selezionata"))
}
struct Plan{root:PathBuf,files:Vec<(PathBuf,PathBuf)>,source:Option<xf::vfs::DataSet>}
fn plan(root:&Path)->Result<Plan,String>{
    let files=candidates(root)?;
    if files.is_empty(){return Ok(Plan{root:root.to_path_buf(),files,source:None});}
    let search=std::env::var_os("XFILES_DISC_DIRECTORY").map(PathBuf::from)
        .unwrap_or_else(||root.parent().unwrap_or(root).to_path_buf());
    let mut discs=Vec::new();
    for n in 1..=7{
        let name=format!("The_X-Files_-_The_Game_CD{n}.iso");
        let image=child(&search,&name).or_else(||child(root,&name))
            .ok_or_else(||format!("La cartella contiene resource fork Macintosh al posto dei dati completi. Per ripristinarli manca {name} accanto alla cartella del gioco. Copia i dati completi dei CD oppure rendi disponibili i sette ISO originali."))?;
        discs.push(image);
    }
    let source=xf::vfs::DataSet::open(&discs)?;
    for name in ["XFILES.HDB","XFILES.GAM"]{
        if source.read(Path::new(name)).map_err(|e|e.to_string())?!=database(root,name)?{
            return Err("Gli ISO non corrispondono alla versione del gioco selezionata. Nessun file e stato ripristinato.".into());
        }
    }
    // Check every source mapping before replacing the first file.
    for (_,key) in &files{source.size(key).map_err(|e|format!("Ripristino non disponibile per {}: {e}",key.display()))?;}
    Ok(Plan{root:root.to_path_buf(),files,source:Some(source)})
}

// Public read-only preflight used by the diagnostic utility.
pub fn inspect(root:&Path)->Result<usize,String>{
    let root=root.canonicalize().map_err(|e|e.to_string())?;Ok(plan(&root)?.files.len())
}
fn replace(root:&Path,relative:&Path,correct:&[u8])->Result<(),String>{
    if resource_fork(correct){return Err("Il CD contiene a sua volta un resource fork, ripristino rifiutato".into());}
    let target=root.join(relative);
    let canonical=target.canonicalize().map_err(|e|e.to_string())?;
    if !canonical.starts_with(root){return Err("File fuori dalla cartella selezionata".into());}
    let original=std::fs::read(&target).map_err(|e|e.to_string())?;
    if !resource_fork(&original){return Ok(());}
    let backup=root.join(BACKUP).join(relative);
    std::fs::create_dir_all(backup.parent().ok_or("Backup path unavailable")?).map_err(|e|e.to_string())?;
    // Never overwrite a different earlier backup.
    match std::fs::read(&backup){
        Ok(bytes) if bytes!=original=>return Err(format!("Copia di sicurezza differente gia presente: {}",backup.display())),
        Ok(_)=>{},
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
            let mut b=std::fs::OpenOptions::new().write(true).create_new(true).open(&backup).map_err(|e|e.to_string())?;
            b.write_all(&original).and_then(|_|b.sync_all()).map_err(|e|e.to_string())?;
        }
        Err(e)=>return Err(e.to_string()),
    }
    let name=target.file_name().ok_or("File name unavailable")?.to_string_lossy();
    let temp=target.with_file_name(format!(".xfiles-restore-{}-{name}.tmp",std::process::id()));
    let result=(||->Result<(),String>{
        let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
        f.write_all(correct).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;drop(f);
        std::fs::rename(&temp,&target).map_err(|e|e.to_string())?;Ok(())
    })();
    if result.is_err(){let _=std::fs::remove_file(temp);}result
}
pub fn repair(root:&Path,progress:&mut dyn FnMut(usize,usize)->bool)->Result<usize,String>{
    let p=plan(root)?;let count=p.files.len();
    let Some(source)=p.source else{return Ok(0);};
    for (i,(relative,key)) in p.files.iter().enumerate(){
        if !progress(i,count){return Err("Ripristino interrotto. I file gia recuperati e le copie di sicurezza sono conservati.".into());}
        let bytes=source.read(key).map_err(|e|e.to_string())?;replace(&p.root,relative,&bytes)?;
    }
    progress(count,count);eprintln!("XFILES restored {count} resource forks in {}",root.display());Ok(count)
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn nonempty_quicktime_resource_fork_requires_bounded_mirrored_map(){
        let mut bytes=vec![0;748];
        let header=[0,0,1,0,0,0,2,166,0,0,1,166,0,0,0,70];
        bytes[..16].copy_from_slice(&header);bytes[678..694].copy_from_slice(&header);
        assert!(resource_fork(&bytes));
        bytes[678]=1;assert!(!resource_fork(&bytes));bytes[678]=0;
        assert!(!resource_fork(&bytes[..747]));
        bytes[8]=255;assert!(!resource_fork(&bytes));
    }
    #[test]fn nested_install_media_keeps_logical_cd_paths_and_valid_286_byte_files(){
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("xfiles-nested-restore-{stamp}"));
        std::fs::create_dir_all(root.join("MININST/XT")).unwrap();
        std::fs::create_dir_all(root.join("MEDINST/XV")).unwrap();
        let mut fork=vec![0;286];fork[..16].copy_from_slice(&HEADER);fork[256..272].copy_from_slice(&HEADER);
        std::fs::write(root.join("MININST/XT/line.XTX"),&fork).unwrap();
        std::fs::write(root.join("MEDINST/XV/clip.XMV"),&fork).unwrap();
        let mut valid=vec![0;286];valid[..4].copy_from_slice(b"HSPT");
        std::fs::write(root.join("MEDINST/XV/valid.HOT"),&valid).unwrap();
        let files=candidates(&root).unwrap();assert_eq!(files.len(),2);
        assert!(files.contains(&(PathBuf::from("MININST/XT/line.XTX"),PathBuf::from("XT/line.XTX"))));
        assert!(files.contains(&(PathBuf::from("MEDINST/XV/clip.XMV"),PathBuf::from("XV/clip.XMV"))));
        assert_eq!(std::fs::read(root.join("MEDINST/XV/valid.HOT")).unwrap(),valid);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn restoration_preserves_originals_and_installer_metadata(){
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("xfiles-restore-{stamp}"));std::fs::create_dir_all(root.join("MININST")).unwrap();
        let root=root.canonicalize().unwrap();let mut fork=vec![0;286];fork[..16].copy_from_slice(&HEADER);fork[256..272].copy_from_slice(&HEADER);
        let relative=Path::new("MININST/X.PFF");std::fs::write(root.join(relative),&fork).unwrap();
        std::fs::write(root.join("LAYOUT.BIN"),b"installer metadata").unwrap();
        assert_eq!(candidates(&root).unwrap(),vec![(relative.to_path_buf(),PathBuf::from("X.PFF"))]);
        replace(&root,relative,b"PFF recovered data").unwrap();
        assert_eq!(std::fs::read(root.join(relative)).unwrap(),b"PFF recovered data");
        assert_eq!(std::fs::read(root.join(BACKUP).join(relative)).unwrap(),fork);
        replace(&root,relative,b"must not overwrite valid data").unwrap();
        assert_eq!(std::fs::read(root.join(relative)).unwrap(),b"PFF recovered data");
        assert_eq!(std::fs::read(root.join("LAYOUT.BIN")).unwrap(),b"installer metadata");
        assert!(candidates(&root).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
