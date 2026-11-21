//! Skip repeated movie-header probing when source files and verified PCM are unchanged.
use std::{path::{Path,PathBuf},time::UNIX_EPOCH};
const MAGIC:&[u8]=b"XFILES-AUDIO-INDEX-1\n";
pub fn media(path:&Path)->bool{
    path.extension().is_some_and(|x|matches!(x.to_string_lossy().to_ascii_lowercase().as_str(),"xmv"|"nmv"|"amv"|"dmv"|"mus"|"mov"))
}
fn snapshot(root:&Path,source:bool)->std::io::Result<Vec<u8>>{
    fn walk(root:&Path,dir:&Path,depth:u8,source:bool,rows:&mut Vec<(PathBuf,u64,u128)>)->std::io::Result<()>{
        for e in std::fs::read_dir(dir)?{
            let e=e?;if e.file_name().to_string_lossy().starts_with('.') {continue;}
            let kind=e.file_type()?;
            if kind.is_symlink(){return Err(std::io::Error::other("Audio index does not follow symbolic links"));}
            if kind.is_dir(){if depth>0{walk(root,&e.path(),depth-1,source,rows)?;}continue;}
            if !kind.is_file() || (source && !media(&e.path())){continue;}
            let m=e.metadata()?;let time=m.modified()?.duration_since(UNIX_EPOCH).map_err(std::io::Error::other)?;
            rows.push((e.path().strip_prefix(root).map_err(std::io::Error::other)?.to_path_buf(),m.len(),time.as_nanos()));
        }Ok(())
    }
    let mut rows=Vec::new();walk(root,root,2,source,&mut rows)?;rows.sort();
    let mut out=Vec::new();
    for (p,len,time) in rows{let p=p.as_os_str().as_encoded_bytes();out.extend((p.len() as u64).to_le_bytes());out.extend(p);out.extend(len.to_le_bytes());out.extend(time.to_le_bytes());}
    Ok(out)
}
pub struct Index{path:PathBuf,cache:PathBuf,source:PathBuf,before:Vec<u8>}
impl Index{
    pub fn open(source:&Path,cache:&Path)->Option<Self>{
        let key=xf::media::pcmcache::fnv64(source.as_os_str().as_encoded_bytes());
        Some(Self{path:cache.join(format!("audio-index-{key:016x}.bin")),cache:cache.join(xf::media::pcmcache::SUBDIR),source:source.to_path_buf(),before:snapshot(source,true).ok()?})
    }
    fn stamp(&self,count:u64)->std::io::Result<Vec<u8>>{
        let cache=snapshot(&self.cache,false)?;
        let mut out=MAGIC.to_vec();out.extend(count.to_le_bytes());out.extend((self.before.len() as u64).to_le_bytes());out.extend(&self.before);out.extend(cache);Ok(out)
    }
    pub fn verified(&self)->Option<u64>{
        let m=std::fs::metadata(&self.path).ok()?;if m.len()>16*1024*1024{return None;}
        let old=std::fs::read(&self.path).ok()?;
        if !old.starts_with(MAGIC){return None;}
        let count=u64::from_le_bytes(old.get(MAGIC.len()..MAGIC.len()+8)?.try_into().ok()?);
        (self.stamp(count).ok()?==old).then_some(count)
    }
    pub fn commit(&self,count:u64){
        // An edited source during conversion must not become a verified index.
        if snapshot(&self.source,true).ok().as_ref()!=Some(&self.before){return;}
        let Ok(bytes)=self.stamp(count)else{return;};
        let temp=self.path.with_extension(format!("{}.tmp",std::process::id()));
        if std::fs::write(&temp,bytes).and_then(|_|std::fs::rename(&temp,&self.path)).is_err(){let _=std::fs::remove_file(temp);}
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn index_invalidates_changed_sources_deleted_pcm_and_different_editions(){
        let id=std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("xfiles-audio-index-{id}"));
        let source=root.join("game");let cache=root.join("cache");let pcm=cache.join(xf::media::pcmcache::SUBDIR);
        std::fs::create_dir_all(source.join("MININST/XV")).unwrap();std::fs::create_dir_all(pcm.join("xv")).unwrap();
        let movie=source.join("MININST/XV/clip.XMV");let wav=pcm.join("xv/clip.wav");
        std::fs::write(&movie,b"source").unwrap();std::fs::write(&wav,b"verified pcm").unwrap();
        let first=Index::open(&source,&cache).unwrap();assert!(first.verified().is_none());first.commit(1);
        assert_eq!(Index::open(&source,&cache).unwrap().verified(),Some(1));
        // Same-size edits still invalidate the index by modification time.
        std::fs::write(&movie,b"edited").unwrap();
        std::fs::File::options().write(true).open(&movie).unwrap().set_modified(std::time::SystemTime::now()+std::time::Duration::from_secs(5)).unwrap();
        assert!(Index::open(&source,&cache).unwrap().verified().is_none());
        Index::open(&source,&cache).unwrap().commit(1);
        let path=Index::open(&source,&cache).unwrap().path;std::fs::write(&path,b"broken index").unwrap();
        assert!(Index::open(&source,&cache).unwrap().verified().is_none());
        std::fs::write(&movie,b"new movie data").unwrap();assert!(Index::open(&source,&cache).unwrap().verified().is_none());
        Index::open(&source,&cache).unwrap().commit(1);std::fs::write(&wav,b"truncated").unwrap();
        assert!(Index::open(&source,&cache).unwrap().verified().is_none());
        Index::open(&source,&cache).unwrap().commit(1);std::fs::remove_file(&wav).unwrap();
        assert!(Index::open(&source,&cache).unwrap().verified().is_none());
        std::fs::write(&wav,b"verified again").unwrap();Index::open(&source,&cache).unwrap().commit(1);
        std::fs::write(source.join("new.AMV"),b"new track").unwrap();assert!(Index::open(&source,&cache).unwrap().verified().is_none());
        let other=root.join("italian");std::fs::create_dir_all(&other).unwrap();assert!(Index::open(&other,&cache).unwrap().verified().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
