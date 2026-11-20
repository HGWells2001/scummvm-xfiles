// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data integration check: natural intro end, skipped intro, menu and New.
use std::{ffi::{CString,CStr},path::Path};
use xfiles_shiloh::*;
fn main(){
    let args:Vec<String>=std::env::args().collect();
    if args.len()!=4{eprintln!("Usage: xfiles-menu-check DATA SAVES OUTPUT_DIRECTORY");std::process::exit(2);}
    std::fs::create_dir_all(&args[3]).unwrap();
    for skip in [false,true]{unsafe{
        let data=CString::new(args[1].as_str()).unwrap();let saves=CString::new(args[2].as_str()).unwrap();
        let mut error=[0i8;1024];let core=xfs_create(data.as_ptr(),saves.as_ptr(),0,error.as_mut_ptr(),error.len());
        if core.is_null(){panic!("{}",CStr::from_ptr(error.as_ptr()).to_string_lossy());}
        let mut frame=vec![0;640*480*3];
        if skip{for _ in 0..4{assert_eq!(xfs_pointer(core,1,320,240),0);assert_eq!(xfs_pointer(core,2,320,240),0);}}
        let mut elapsed=0;
        while !menu_ready(core) && elapsed<200_000{
            assert_eq!(xfs_tick(core,250),0);assert_eq!(xfs_frame(core,frame.as_mut_ptr(),frame.len()),0);elapsed+=250;
        }
        assert!(menu_ready(core),"Intro never reached the interactive menu");
        assert_eq!(xfs_frame(core,frame.as_mut_ptr(),frame.len()),0);
        let right=frame.chunks_exact(640*3).flat_map(|row|row[472*3..].iter()).filter(|&&v|v!=0).count();
        let left=frame.chunks_exact(640*3).flat_map(|row|row[..472*3].iter()).filter(|&&v|v!=0).count();
        let name=if skip{"menu-skipped.ppm"}else{"menu-natural.ppm"};
        let mut ppm=b"P6\n640 480\n255\n".to_vec();ppm.extend(&frame);std::fs::write(Path::new(&args[3]).join(name),ppm).unwrap();
        assert!(left>10_000 && right>1000,"Menu artwork missing: left={left}, right={right}");
        assert_eq!(xfs_pointer(core,0,570,101),0);assert_eq!(xfs_pointer(core,1,570,101),0);assert_eq!(xfs_pointer(core,2,570,101),0);
        assert_eq!(xfs_tick(core,16),0);assert_eq!(xfs_view(core),691,"New did not start the first scene");
        println!("PASS: skip={skip}, menu after {elapsed}ms, left={left}, labels={right}, New view=691");
        xfs_destroy(core);
    }}
}
