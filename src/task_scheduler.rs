use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Instant,Duration};

pub type Task<T> = Box<dyn FnOnce() -> T + Send + 'static>;

trait Executable {
    fn execute(
        self: Box<Self>,
        time_cost_table: Arc<Mutex<HashMap<u64, Duration>>>,
        function_id: u64,
    );
}

impl<T: Send + 'static> Executable for (Box<dyn FnOnce() -> T + Send + 'static>, mpsc::Sender<T>) {
    fn execute(
        self: Box<Self>,
        time_cost_table: Arc<Mutex<HashMap<u64, Duration>>>,
        function_id: u64,
    ) {
        let (task, sender) = *self;
        let start_time = Instant::now();
        let result = task();
        let elapsed_time = start_time.elapsed();
        let _ = sender.send(result);

        let mut map = time_cost_table.lock().unwrap();
        map.insert(function_id, elapsed_time);
    }
}

pub struct TaskWithResult {
    task: Box<dyn Executable + Send>,
    time_cost_table: Arc<Mutex<HashMap<u64, Duration>>>,
    function_id: u64,
}

pub struct Worker {
    kthread_handle: thread::JoinHandle<()>,
}

impl Worker {
    pub fn new(receiver: mpsc::Receiver<TaskWithResult>) -> Self {
        let kthread_handle = thread::spawn(move || {
            for TaskWithResult {
                task,
                time_cost_table,
                function_id,
            } in receiver
            {
                task.execute(time_cost_table.clone(), function_id);
            }
        });

        Worker { kthread_handle }
    }
}

pub struct TaskScheduler {
    senders: Vec<(mpsc::Sender<TaskWithResult>, Instant)>,
    pub time_cost_table: Arc<Mutex<HashMap<u64, Duration>>>,
}

impl TaskScheduler {
    pub fn new(worker_count: usize) -> Self {
        let mut senders = Vec::with_capacity(worker_count);
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (sender, receiver) = mpsc::channel::<TaskWithResult>();
            senders.push((sender, Instant::now()));
            workers.push(Worker::new(receiver));
        }

        TaskScheduler {
            senders,
            time_cost_table: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn schedule_task<T: 'static + Send>(
        &mut self,
        task: impl FnOnce() -> T + Send + 'static,
    ) -> mpsc::Receiver<T> {
        let (result_sender, result_receiver) = mpsc::channel();
        let function_id = &task as *const _ as *const usize as u64;
        let task = Box::new(task);

        let task_with_result = TaskWithResult {
            task: Box::new((
                task as Box<dyn FnOnce() -> T + Send + 'static>,
                result_sender,
            )),
            time_cost_table: Arc::clone(&self.time_cost_table),
            function_id,
        };

        if let Some((sender_index, _)) = self
            .senders
            .iter()
            .enumerate()
            .min_by(|(_, (_, instant1)), (_, (_, instant2))| instant1.cmp(instant2))
        {
            let least_busy = &self.senders[sender_index].0;
            let _ = least_busy.send(task_with_result);
            if let Ok(lock) = self.time_cost_table.lock() {
                if let Some(&function_duration) = lock.get(&function_id) {
                    self.senders[sender_index].1 = Instant::now() + function_duration;
                } else {
                    self.senders[sender_index].1 = Instant::now();
                }
            }
        }

        result_receiver
    }
}
