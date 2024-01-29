use librthreads::task_scheduler::*;

fn main() {
    let mut scheduler: TaskScheduler;
    match std::thread::available_parallelism() {
        Ok(num_cpus) => {
            println!("Number of logical processors: {}", num_cpus.get());
            scheduler = TaskScheduler::new(num_cpus.get());
        }

        Err(e) => {
            println!(
                "Could not determine the number of logical processors: {}",
                e
            );
            scheduler = TaskScheduler::new(1);
        }
    }

    // Schedule tasks to calculate large Fibonacci numbers
    let result_receivers = (1..400)
        .map(|i| {
            // Use larger numbers for heavier workload
            let closure = move || {
                println!("Calculating Fibonacci for {}", i);
                fibonacci(i)
            };

            scheduler.schedule_task(closure)
        })
        .collect::<Vec<_>>();

    // Collect results
    for receiver in result_receivers {
        let result = receiver.recv().unwrap();
        println!("Fibonacci result: {:?}", result);
        println!(
            "Table Size: {:?}",
            scheduler.time_cost_table.lock().unwrap().len()
        );
    }
}

// Inefficient recursive Fibonacci function for demonstration
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
