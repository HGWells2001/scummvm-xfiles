// SPDX-License-Identifier: GPL-3.0-or-later
fn main(){
    let Some(path)=std::env::args_os().nth(1)else{eprintln!("Usage: xfiles-folder-check GAME_DIRECTORY");std::process::exit(2);};
    match xfiles_shiloh::inspect_cd_folder(std::path::Path::new(&path)){
        Ok(n)=>println!("Read-only check: {n} empty resource forks; matching CD sources verified when needed"),
        Err(e)=>{eprintln!("{e}");std::process::exit(1);}
    }
}
