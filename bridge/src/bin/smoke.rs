// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::CString;
use xfiles_shiloh::*;
fn main(){
    let args:Vec<String>=std::env::args().collect();
    if args.len()<3{eprintln!("Usage: xfiles-shiloh-smoke DATA_DIRECTORY SAVE_DIRECTORY [FRAME.ppm]");std::process::exit(2);}
    let data=CString::new(args[1].as_str()).unwrap();let saves=CString::new(args[2].as_str()).unwrap();
    let mut err=[0i8;1024];
    unsafe{
        let core=xfs_create(data.as_ptr(),saves.as_ptr(),2,err.as_mut_ptr(),err.len());
        if core.is_null(){eprintln!("{}",std::ffi::CStr::from_ptr(err.as_ptr()).to_string_lossy());std::process::exit(1);}
        let mut frame=vec![0u8;640*480*3];let mut audio=vec![0i16;1470];let mut nonzero_audio=0;
        let mut failed=false;
        for _ in 0..120{
            if xfs_tick(core,16)<0 || xfs_frame(core,frame.as_mut_ptr(),frame.len())<0 || xfs_mix(core,audio.as_mut_ptr(),audio.len())<0{failed=true;break;}
            nonzero_audio+=audio.iter().filter(|&&v|v!=0).count();
        }
        if failed{xfs_error(core,err.as_mut_ptr(),err.len());eprintln!("{}",std::ffi::CStr::from_ptr(err.as_ptr()).to_string_lossy());}
        else{
            println!("Shiloh/ScummVM core smoke: 120 ticks, view={}, lit_bytes={}, nonzero_audio={}",xfs_view(core),frame.iter().filter(|&&v|v!=0).count(),nonzero_audio);
            let mut overlay = vec![0; frame.len()];
            let header = 640 * 3 * 15;
            assert_eq!(xfs_key(core, 0x70, 0), 0);
            assert_eq!(xfs_frame(core, overlay.as_mut_ptr(), overlay.len()), 0);
            assert_ne!(overlay[..header], frame[..header], "F1 should draw hotspot diagnostics");
            assert_eq!(xfs_key(core, 0x70, 0), 0);
            assert_eq!(xfs_frame(core, overlay.as_mut_ptr(), overlay.len()), 0);
            assert_eq!(overlay[..header], frame[..header], "F1 should hide hotspot diagnostics");
            if let Some(path)=args.get(3){let mut b=b"P6\n640 480\n255\n".to_vec();b.extend(&frame);std::fs::write(path,b).unwrap();}
        }
        xfs_destroy(core);if failed{std::process::exit(1);}
    }
}
