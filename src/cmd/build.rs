use std::path::{PathBuf};
use anyhow::{Result};
use serde::Serialize;
use crate::build::worker::{worker_run, WorkerArgument};
use crate::data::{File, FilePath, GeneralHashType, Job, PathTarget, ResultTrait};
use crate::threadpool::ThreadPool;

mod worker;

pub struct BuildSettings {
    pub directory: PathBuf,
    pub into_archives: bool,
    pub follow_symlinks: bool,
    pub output: PathBuf,
    pub absolute_paths: bool,
    pub threads: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
enum JobResult {
    Final(File),
    Intermediate(File),
}

impl ResultTrait for JobResult {
    
}

pub fn run(
    build_settings: BuildSettings,
) -> Result<()> {
    let mut args = Vec::with_capacity(build_settings.threads.unwrap_or_else(|| num_cpus::get()));
    for _ in 0..args.capacity() {
        args.push(WorkerArgument {
            follow_symlinks: build_settings.follow_symlinks,
            hash: GeneralHashType::NULL,
        });
    }
    
    let pool: ThreadPool<Job, JobResult> = ThreadPool::new(args, worker_run);

    let root_file = FilePath::from_path(build_settings.directory, PathTarget::File);
    let root_job = Job::new(None, root_file);
    
    pool.publish(root_job);

    while let Ok(result) = pool.receive() {
        // print as json
        serde_json::to_writer_pretty(std::io::stdout(), &result)?;
        
        if let JobResult::Final(_) = result {
            break;
        }
    }
    
    return Ok(());
}












    
    /*

    let inside_scope = |path: &'_ Path| -> bool { true };
    let lookup_id = |id: &'_ HandleIdentifier| -> Result<anyhow::Error> { Err(anyhow!("lookup_id")) };

    let root = File::new(build_settings.directory);
    let root = Rc::new(RefCell::new(FileContainer::InMemory(RefCell::new(root))));

    let pid = std::process::id();
    let system = sysinfo::System::new_all();
    let current_process = system.process(Pid::from_u32(pid)).expect("Failed to get current process");
    let current_memory_usage = current_process.memory();

    println!("Current memory usage: {}", current_memory_usage);

    analyze_file(Rc::clone(&root));

    let system = sysinfo::System::new_all();
    let current_process = system.process(Pid::from_u32(pid)).expect("Failed to get current process");

    println!("Mem before: {}", current_memory_usage);
    println!("Mem after:  {}", current_process.memory());

    let json_str = serde_json::to_string_pretty(&root)?;
    //println!("{}", json_str);

    Ok(())
}

fn analyze_file(root: Rc<RefCell<FileContainer>>) {
    let inside_scope = |path: &'_ Path| -> bool { true };
    let lookup_id = |id: &'_ HandleIdentifier| -> Result<anyhow::Error> { Err(anyhow!("lookup_id")) };

    let mut stack = Vec::with_capacity(250);
    stack.push(root);

    while let Some(stack_next) = stack.pop() {
        let stack_next_borrow = stack_next.borrow();
        match stack_next_borrow.deref() {
            FileContainer::InMemory(file) => {
                let mut file_borrow = file.borrow_mut();
                match file_borrow.deref_mut() {
                    File::Directory(ref mut dir) => {
                        if dir.children.len() == 0 {
                            if log_enabled!(Level::Info) {
                                info!("Analyzing directory: {:?}", dir.path);
                            }

                            dir.analyze_expand(/*true, inside_scope, lookup_id*/);

                            // for all children
                            if dir.children.len() > 0 {
                                stack.push(Rc::clone(&stack_next));
                                for child in dir.children.iter() {
                                    stack.push(Rc::clone(&child));
                                }
                            } else {
                                dir.analyze_collect();
                            }
                        } else {
                            dir.analyze_collect();
                        }
                    },
                    File::File(ref mut file) => {
                        file.analyze();
                    },
                    File::Other(_) => { /* no analysis needed */ },
                    File::Symlink(ref mut file) => {
                        file.analyze(/*lookup_id*/);
                    }
                }
            },
            FileContainer::OnDisk(_) => {
                todo!("Unloading files from memory to disk not yet supported");
            },
        }
    }

}
*/