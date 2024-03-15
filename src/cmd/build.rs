use std::{fs, thread};
use std::ops::{Deref, DerefMut};
use std::path::{PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, LockResult, MutexGuard};
use std::time::SystemTime;
use anyhow::{anyhow, Result};
use log::{error, info, trace, warn};
use crate::data::{DirectoryInformation, File, FileInformation, FilePath, GeneralHash, GeneralHashType, Job, JobState, JobTrait, OtherInformation, PathTarget, ResultTrait, SymlinkInformation};
use crate::data::JobState::NotProcessed;
use crate::threadpool::ThreadPool;
use crate::utils;

pub struct BuildSettings {
    pub directory: PathBuf,
    pub into_archives: bool,
    pub follow_symlinks: bool,
    pub output: PathBuf,
    pub absolute_paths: bool,
    pub threads: Option<usize>,
}

impl ResultTrait for File {
    
}

struct WorkerArgument {
    follow_symlinks: bool,

    hash: GeneralHashType,
}

pub fn run(
    build_settings: BuildSettings,
) -> Result<()> {
    let mut args = Vec::with_capacity(build_settings.threads.unwrap_or_else(|| num_cpus::get()));
    for _ in 0..args.capacity() {
        args.push(WorkerArgument {
            follow_symlinks: build_settings.follow_symlinks,
            hash: GeneralHashType::SHA256,
        });
    }
    
    let pool: ThreadPool<Job, File> = ThreadPool::new(args, worker_run);

    let root_file = FilePath::from_path(build_settings.directory, PathTarget::File);
    let root_job = Job::new(None, root_file);
    
    pool.publish(root_job);

    let result =  pool.receive()?;
    
    // print as json
    serde_json::to_writer_pretty(std::io::stdout(), &result)?;

    return Ok(());
}

fn worker_publish_result(id: usize, result_publish: &Sender<File>, result: File) {
    match result_publish.send(result) {
        Ok(_) => {},
        Err(e) => {
            warn!("[{}] failed to publish result: {}", id, e);
        }
    }
}

fn worker_create_error(path: FilePath) -> File {
    File::Other(OtherInformation {
        path,
    })
}

fn worker_publish_error(id: usize, result_publish: &Sender<File>, path: FilePath) {
    let result = worker_create_error(path);
    worker_publish_result(id, result_publish, result);
}

fn worker_publish_new_job(id: usize, job_publish: &Sender<Job>, job: Job) {
    match job_publish.send(job) {
        Ok(_) => {},
        Err(e) => {
            warn!("[{}] failed to publish job: {}", id, e);
        }
    }
}

fn worker_publish_result_or_trigger_parent(id: usize, result: File, job: Job, result_publish: &Sender<File>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    let unfinished_children: usize;
    let parent_job;

    match job.parent {
        Some(parent) => {
            parent_job = Arc::clone(&parent);
            let parent = parent.deref();
            match parent.unfinished_children.lock() {
                Ok(mut unfinshed_childrens) => {
                    let deref_mut = unfinshed_childrens.deref_mut();
                    if *deref_mut <= 0 {
                        error!("[{}] Parent job has no unfinished children", id);
                        return;
                    }
                    *deref_mut -= 1;
                    unfinished_children = *deref_mut;

                    match parent.finished_children.lock() {
                        Ok(mut finished_children) => {
                            finished_children.push(result);
                        },
                        Err(e) => {
                            error!("[{}] failed to lock finished children: {}", id, e);
                            return;
                        }
                    }
                },
                Err(e) => {
                    error!("[{}] failed to lock children count: {}", id, e);
                    return;
                }
            }
        },
        None => {
            worker_publish_result(id, result_publish, result);
            return;
        },
    }
    
    let target_peth = job.target_path;

    if unfinished_children <= 0 {
        match Arc::into_inner(parent_job) {
            Some(parent_job) => {
                worker_publish_new_job(id, job_publish, parent_job);
            },
            None => {
                error!("[{}] failed to convert parent job", id);
                worker_publish_error(id, result_publish, target_peth);
            }
        }
    }
}

fn worker_run_symlink(path: PathBuf, modified: u64, id: usize, job: Job, result_publish: &Sender<File>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing symlink {:?}#{:?}", id, &job.target_path, path);
    let target_link = fs::read_link(&path);
    let target_link = match target_link {
        Ok(target_link) => target_link,
        Err(err) => {
            error!("Error while reading symlink {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    };

    let mut hash = GeneralHash::from_type(arg.hash);
    
    match utils::hash_path(&target_link, &mut hash) {
        Ok(_) => {},
        Err(err) => {
            error!("Error while hashing symlink target {:?}: {}", target_link, err);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    }
    
    let file = File::Symlink(SymlinkInformation {
        path: job.target_path.clone(),
        modified,
        content_hash: hash,
        target: target_link,
    });

    worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
}

fn worker_run_directory(path: PathBuf, modified: u64, id: usize, mut job: Job, result_publish: &Sender<File>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing directory {:?}#{:?}", id, &job.target_path, path);
    match job.state {
        NotProcessed => {
            let read_dir = fs::read_dir(&path);
            let read_dir = match read_dir {
                Ok(read_dir) => read_dir,
                Err(err) => {
                    error!("Error while reading directory {:?}: {}", path, err);
                    worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                    return;
                }
            };

            let mut children = Vec::new();

            for entry in read_dir {
                match entry {
                    Ok(entry) => {
                        let child_path = job.target_path.child_real(entry.file_name());
                        children.push(child_path);
                    },
                    Err(err) => {
                        error!("Error while reading directory entry {:?}: {}", path, err);
                    }
                };
            }

            job.state = JobState::Analyzed;
            let error;
            match job.unfinished_children.lock() {
                Ok(mut unfinished_children) => {
                    *unfinished_children = children.len();
                    error = false;
                },
                Err(e) => {
                    error = true;
                    error!("[{}] failed to lock children count: {}", id, e);
                    return;
                }
            }
            if error {
                worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                return;
            }
            
            let parent_job = Arc::new(job);
            let mut jobs = Vec::with_capacity(children.len());

            for child in children {
                let job = Job::new(Some(Arc::clone(&parent_job)), child);
                jobs.push(job);
            }

            drop(parent_job);

            for job in jobs {
                match job_publish.send(job) {
                    Ok(_) => {},
                    Err(e) => {
                        error!("[{}] failed to publish job: {}", id, e);
                    }
                }
            }
        },
        JobState::Analyzed => {
            let mut hash = GeneralHash::from_type(arg.hash);
            let mut children = Vec::new();
            
            let mut error;
            match job.finished_children.lock() {
                Ok(mut finished) => {
                    error = false;
                    match utils::hash_directory(finished.iter(), &mut hash) {
                        Ok(_) => {},
                        Err(err) => {
                            error = true;
                            error!("Error while hashing directory {:?}: {}", path, err);
                        }
                    }
                    children.append(finished.deref_mut());
                }
                Err(err) => {
                    error!("[{}] failed to lock finished children: {}", id, err);
                    error = true;
                }
            }
            if (error) {
                worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                return;
            }
            
            let file = File::Directory(DirectoryInformation {
                path: job.target_path.clone(),
                modified,
                content_hash: hash,
                number_of_children: children.len() as u64,
                children,
            });

            worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
        }
    }
}

fn worker_run_file(path: PathBuf, modified: u64, id: usize, job: Job, result_publish: &Sender<File>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing file {:?}#{:?}", id, &job.target_path, path);
    match fs::File::open(&path) {
        Ok(file) => {
            let mut reader = std::io::BufReader::new(file);
            let mut hash = GeneralHash::from_type(arg.hash);
            let content_size;
            match utils::hash_file(&mut reader, &mut hash) {
                Ok(size) => {
                    content_size = size;
                }
                Err(err) => {
                    error!("Error while hashing file {:?}: {}", path, err);
                    worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                    return;
                }
            }

            let file = File::File(FileInformation {
                path: job.target_path.clone(),
                modified,
                content_hash: hash,
                content_size,
            });
            worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
            return;
        }
        Err(err) => {
            error!("Error while opening file {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    }
}

fn worker_run_other(path: PathBuf, modified: u64, id: usize, job: Job, result_publish: &Sender<File>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing other {:?}#{:?}", id, &job.target_path, path);
    let file = File::Other(OtherInformation {
        path: job.target_path.clone(),
    });

    worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
}

fn worker_run(id: usize, job: Job, result_publish: &Sender<File>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    let path = job.target_path.resolve_file();
    let path = match path {
        Ok(file) => file,
        Err(e) => {
            error!("[{}] failed to resolve file: {}", id, e);
            info!("[{}] Skipping file...", id);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    };
    
    let metadata = match arg.follow_symlinks {
        true => fs::metadata(&path),
        false => fs::symlink_metadata(&path),
    };
    
    let metadata = match metadata {
        Ok(metadata) => metadata,
        Err(e) => {
            warn!("[{}] failed to read metadata: {}", id, e);
            info!("[{}] Skipping file...", id);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    };

    let modified_result = metadata.modified()
        .map(|time| time.duration_since(SystemTime::UNIX_EPOCH)
            .or(Err(anyhow!("Unable to convert modified date to UNIX_EPOCH")))
            .map(|duration| duration.as_secs())
        ).unwrap_or_else(|err| {
        error!("Error while reading modified date {:?}: {:?}", path, err);
        Ok(0)
    });

    let modified;

    match modified_result {
        Ok(time) => modified = time,
        Err(err) => {
            error!("Error while processing file {:?}: {}", path, err);
            modified = 0;
        }
    }

    if metadata.is_symlink() {
        worker_run_symlink(path, modified, id, job, result_publish, job_publish, arg);
    } else if metadata.is_dir() {
        worker_run_directory(path, modified, id, job, result_publish, job_publish, arg);
    } else if metadata.is_file() {
        worker_run_file(path, modified, id, job, result_publish, job_publish, arg);
    } else {
        worker_run_other(path, modified, id, job, result_publish, job_publish, arg);
    }
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