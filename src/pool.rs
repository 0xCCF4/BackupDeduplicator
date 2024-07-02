use log::{debug, error, trace, warn};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A trait that must be implemented by a job type to be processed by the pool.
pub trait JobTrait<T: Send = Self> {
    /// Get the job id.
    ///
    /// # Returns
    /// * `usize` - The job id.
    fn job_id(&self) -> usize;
}

/// A trait that must be implemented by a result type to be returned by the pool.
pub trait ResultTrait<T: Send = Self> {}

/// Worker entry function signature
/// The worker entry function is called by the worker thread to process a job.
/// A custom worker must supply a function of this type to the thread pool to process jobs.
///
/// # Arguments
/// * `usize` - The current worker id.
/// * `Job` - The job received that should be processed.
/// * `&Sender<Result>` - A sender to publish job results.
/// * `&Sender<Job>` - A sender to publish new jobs to the thread pool.
/// * `&mut Argument` - A mutable reference to the arguments passed to the worker thread via the thread pool creation.
///
/// # Returns
/// * `()` - The worker entry function should not return a value but instead should send the result via the `Sender<Result>` back to the main thread.
type WorkerEntry<Job, Result, Argument> =
    fn(usize, Job, &Sender<Result>, &Sender<Job>, &mut Argument);

/// Internal worker struct to manage the worker thread via the thread pool.
///
/// # Fields
/// * `id` - The worker id.
/// * `thread` - The worker thread handle.
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Create a new worker thread. Starts the worker thread and returns the worker struct.
    ///
    /// # Arguments
    /// * `id` - The worker id.
    /// * `job_receive` - A receiver to receive jobs from the thread pool.
    /// * `result_publish` - A sender to publish job results.
    /// * `job_publish` - A sender to publish new jobs to the thread pool.
    /// * `func` - The worker entry function to process jobs.
    /// * `arg` - The arguments passed to the worker thread via the thread pool creation.
    ///
    /// # Returns
    /// * `Worker` - The worker struct with the worker thread handle.
    fn new<
        Job: JobTrait + Send + 'static,
        Result: ResultTrait + Send + 'static,
        Argument: Send + 'static,
    >(
        id: usize,
        job_receive: Arc<Mutex<Receiver<Job>>>,
        result_publish: Sender<Result>,
        job_publish: Sender<Job>,
        func: WorkerEntry<Job, Result, Argument>,
        arg: Argument,
    ) -> Worker {
        let thread = thread::spawn(move || {
            Worker::worker_entry(id, job_receive, result_publish, job_publish, func, arg);
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }

    /// Function executed by the worker thread. Does exit when the job receiver is closed/the thread pool is shutting down.
    ///
    /// # Arguments
    /// * `id` - The worker id.
    /// * `job_receive` - A receiver to receive jobs from the thread pool.
    /// * `result_publish` - A sender to publish job results.
    /// * `job_publish` - A sender to publish new jobs to the thread pool.
    /// * `func` - The worker entry function to process jobs.
    /// * `arg` - The arguments passed to the worker thread via the thread pool creation.
    fn worker_entry<
        Job: JobTrait + Send + 'static,
        Result: ResultTrait + Send + 'static,
        Argument: Send + 'static,
    >(
        id: usize,
        job_receive: Arc<Mutex<Receiver<Job>>>,
        result_publish: Sender<Result>,
        job_publish: Sender<Job>,
        func: WorkerEntry<Job, Result, Argument>,
        mut arg: Argument,
    ) {
        loop {
            // Acquire the job lock
            let job = job_receive.lock();

            let job = match job {
                Err(e) => {
                    error!("Worker {} shutting down {}", id, e);
                    break;
                }
                Ok(job) => {
                    job.recv() // receive new job
                }
            };

            match job {
                Err(_) => {
                    trace!("Worker {} shutting down", id);
                    break;
                }
                Ok(job) => {
                    trace!("Worker {} received job {}", id, job.job_id());
                    // Call the user function to process the job
                    func(id, job, &result_publish, &job_publish, &mut arg);
                }
            }
        }
    }
}

/// A thread pool to manage the distribution of jobs to worker threads.
///
/// # Template Parameters
/// * `Job` - The job type that should be processed by the worker threads.
/// * `Result` - The result type that should be returned by the worker threads.
///
/// Both `Job` and `Result` must implement the `Send` trait.
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

impl<Job: Send + JobTrait + 'static, Result: Send + ResultTrait + 'static> ThreadPool<Job, Result> {
    /// Create a new thread pool with a given number of worker threads (args.len()).
    /// Each worker thread will receive an argument from the args vector. When a new job
    /// is published to the thread pool, the thread pool will distribute the job to the worker threads
    /// and execute the `func` function within a worker thread.
    ///
    /// # Arguments
    /// * `args` - A vector of arguments that should be passed to the worker threads.
    /// * `func` - The worker entry function to process jobs.
    ///
    /// # Returns
    /// * `ThreadPool` - The thread pool struct with the worker threads.
    ///
    /// # Template Parameters
    /// * `Argument` - The argument type that should be passed to the worker threads.
    /// The argument type must implement the `Send` trait.
    pub fn new<Argument: Send + 'static>(
        mut args: Vec<Argument>,
        func: WorkerEntry<Job, Result, Argument>,
    ) -> ThreadPool<Job, Result> {
        assert!(args.len() > 0);

        let mut workers = Vec::with_capacity(args.len());

        let (job_publish, job_receive) = mpsc::channel();

        let job_receive = Arc::new(Mutex::new(job_receive));
        let (result_publish, result_receive) = mpsc::channel();
        let (thread_publish_job, thread_receive_job) = mpsc::channel();

        let mut id = 0;
        while let Some(arg) = args.pop() {
            workers.push(Worker::new(
                id,
                Arc::clone(&job_receive),
                result_publish.clone(),
                thread_publish_job.clone(),
                func,
                arg,
            ));
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

    /// Publish a new job to the thread pool. The job will be distributed to a worker thread.
    ///
    /// # Arguments
    /// * `job` - The job that should be processed by a worker thread.
    pub fn publish(&self, job: Job) {
        let job_publish = self.job_publish.lock();
        match job_publish {
            Err(e) => {
                error!("ThreadPool is shutting down. Cannot publish job. {}", e);
            }
            Ok(job_publish) => match job_publish.as_ref() {
                None => {
                    error!("ThreadPool is shutting down. Cannot publish job.");
                }
                Some(job_publish) => match job_publish.send(job) {
                    Err(e) => {
                        error!("Failed to publish job on thread pool. {}", e);
                    }
                    Ok(_) => {}
                },
            },
        }
    }

    /// Internal function that is run in a separate thread. It feeds back jobs from the worker threads to the input of the thread pool.
    ///
    /// # Arguments
    /// * `job_publish` - A sender to publish new jobs to the thread pool.
    /// * `job_receive` - A receiver to receive jobs from the worker threads.
    fn pool_entry(job_publish: Arc<Mutex<Option<Sender<Job>>>>, job_receive: Receiver<Job>) {
        loop {
            let job = job_receive.recv();

            match job {
                Err(_) => {
                    trace!("Pool worker shutting down");
                    break;
                }
                Ok(job) => match job_publish.lock() {
                    Err(e) => {
                        error!("Pool worker shutting down: {}", e);
                        break;
                    }
                    Ok(job_publish) => {
                        if let Some(job_publish) = job_publish.as_ref() {
                            job_publish
                                .send(job)
                                .expect("Pool worker failed to send job. This should never fail.");
                        }
                    }
                },
            }
        }
    }

    /// Receive a result from the worker threads. This function will block until a result is available.
    ///
    /// # Returns
    /// * `Result` - The result of a job processed by a worker thread.
    ///
    /// # Errors
    /// * If all worker threads panicked, therefore the pipe is closed
    pub fn receive(&self) -> std::result::Result<Result, mpsc::RecvError> {
        self.result_receive.recv()
    }

    /// Receive a result from the worker threads. This function will block until a result is available or a timeout occurs.
    ///
    /// # Arguments
    /// * `timeout` - The maximum time to wait for a result.
    ///
    /// # Returns
    /// * `Result` - The result of a job processed by a worker thread.
    ///
    /// # Errors
    /// * If all worker threads panicked, therefore the pipe is closed
    /// * If the timeout occurs before a result is available
    pub fn receive_timeout(
        &self,
        timeout: Duration,
    ) -> std::result::Result<Result, RecvTimeoutError> {
        self.result_receive.recv_timeout(timeout)
    }
}

impl<Job: Send, Result: Send> Drop for ThreadPool<Job, Result> {
    fn drop(&mut self) {
        drop(
            self.job_publish
                .lock()
                .expect("This should not break")
                .take(),
        );

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
