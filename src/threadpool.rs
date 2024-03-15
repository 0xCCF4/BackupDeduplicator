use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;
use log::{debug, error, trace, warn};
use crate::data::{JobTrait, ResultTrait};

type WorkerEntry<Job, Result, Argument> = fn(usize, Job, &Sender<Result>, &Sender<Job>, &mut Argument);

struct Worker
{
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new<Job: JobTrait + std::marker::Send + 'static, Result: ResultTrait + std::marker::Send + 'static, Argument: std::marker::Send + 'static>(id: usize, job_receive: Arc<Mutex<Receiver<Job>>>, result_publish: Sender<Result>, job_publish: Sender<Job>, func: WorkerEntry<Job, Result, Argument>, arg: Argument) -> Worker {
        let thread = thread::spawn(move || {
            Worker::worker_entry(id, job_receive, result_publish, job_publish, func, arg);
        });

        Worker { id, thread: Some(thread) }
    }

    fn worker_entry<Job: JobTrait + std::marker::Send + 'static, Result: ResultTrait + std::marker::Send + 'static, Argument: std::marker::Send + 'static>(id: usize, job_receive: Arc<Mutex<Receiver<Job>>>, result_publish: Sender<Result>, job_publish: Sender<Job>, func: WorkerEntry<Job, Result, Argument>, mut arg: Argument) {
        loop {
            let job = job_receive.lock();

            let job = match job {
                Err(e) => {
                    error!("Worker {} shutting down {}", id, e);
                    break;
                }
                Ok(job) => {
                    job.recv()
                }
            };

            match job {
                Err(_) => {
                    trace!("Worker {} shutting down", id);
                    break;
                }
                Ok(job) => {
                    trace!("Worker {} received job {}", id, job.job_id());
                    func(id, job, &result_publish, &job_publish, &mut arg);
                }
            }
        }
    }
}

pub struct ThreadPool<Job, Result>
where
    Job: Send,
    Result: Send,
{
    workers: Vec<Worker>,
    thread: Option<thread::JoinHandle<()>>,
    job_publish: Arc<Mutex<Option<Sender<Job>>>>,
    result_receive: Receiver<Result>,
}

impl<Job: std::marker::Send + JobTrait + 'static, Result: std::marker::Send + ResultTrait + 'static> ThreadPool<Job, Result> {
    pub fn new<Argument: std::marker::Send + 'static>(mut args: Vec<Argument>, func: WorkerEntry<Job, Result, Argument>) -> ThreadPool<Job, Result> {
        assert!(args.len() > 0);

        let mut workers = Vec::with_capacity(args.len());

        let (job_publish, job_receive) = mpsc::channel();

        let job_receive = Arc::new(Mutex::new(job_receive));
        let (result_publish, result_receive) = mpsc::channel();
        let (thread_publish_job, thread_receive_job) = mpsc::channel();

        let mut id = 0;
        while let Some(arg) = args.pop() {
            workers.push(Worker::new(id, Arc::clone(&job_receive), result_publish.clone(), thread_publish_job.clone(), func, arg));
            id += 1;
        }

        let job_publish = Arc::new(Mutex::new(Some(job_publish)));
        let job_publish_clone = Arc::clone(&job_publish);

        let thread = thread::spawn(move || {
            ThreadPool::<Job, Result>::pool_entry(job_publish_clone, thread_receive_job);
        });

        ThreadPool {
            workers,
            job_publish,
            result_receive,
            thread: Some(thread),
        }
    }
    
    pub fn publish(&self, job: Job) {
        let job_publish = self.job_publish.lock();
        match job_publish {
            Err(e) => {
                error!("ThreadPool is shutting down. Cannot publish job. {}", e);
            }
            Ok(job_publish) => {
                match job_publish.as_ref() {
                    None => {
                        error!("ThreadPool is shutting down. Cannot publish job.");
                    }
                    Some(job_publish) => {
                        match job_publish.send(job) {
                            Err(e) => {
                                error!("Failed to publish job on thread pool. {}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                }
            }
        }

    }

    fn pool_entry(job_publish: Arc<Mutex<Option<Sender<Job>>>>, job_receive: Receiver<Job>) {
        loop {
            let job = job_receive.recv();

            match job {
                Err(_) => {
                    trace!("Pool worker shutting down");
                    break;
                }
                Ok(job) => {
                    match job_publish.lock() {
                        Err(e) => {
                            error!("Pool worker shutting down: {}", e);
                            break;
                        }
                        Ok(job_publish) => {
                            if let Some(job_publish) = job_publish.as_ref() {
                                job_publish.send(job).expect("Pool worker failed to send job. This should never fail.");
                            }
                        }
                    }
                }
            }
        }
    }
    
    pub fn receive(&self) -> std::result::Result<Result, mpsc::RecvError> {
        self.result_receive.recv()
    }

    pub fn receive_timeout(&self, timeout: Duration) -> std::result::Result<Result, RecvTimeoutError> {
        self.result_receive.recv_timeout(timeout)
    }
}

impl<Job: std::marker::Send, Result: std::marker::Send> Drop for ThreadPool<Job, Result> {
    fn drop(&mut self) {
        drop(self.job_publish.lock().expect("This should not break").take());

        for worker in &mut self.workers {
            debug!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                match thread.join() {
                    Ok(_) => {
                        trace!("Worker {} shut down", worker.id);
                    }
                    Err(_) => {
                        warn!("Worker {} panicked", worker.id);
                    }
                }
            }
        }

        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(_) => {
                    trace!("ThreadPool shut down");
                }
                Err(_) => {
                    warn!("ThreadPool worker panicked");
                }
            }
        }
    }
}