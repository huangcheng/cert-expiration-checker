use std::env;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::{available_parallelism, sleep, JoinHandle};
use std::fs::read_to_string;
use std::ops::Add;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use env::current_dir;
use std::process::exit;
use chrono::{TimeDelta, Utc};
use clap::Parser;
use ssl_expiration2::SslExpiration;
use cli_table::{format::Justify, print_stdout, Cell, Style, Table};

mod cli;

use cli::Cli;

fn get_file() -> PathBuf {
    let cli = Cli::parse();

    if let Some(file) = cli.file.as_deref() {
        return PathBuf::from(file);
    }

    let cwd = current_dir();

    let cwd = cwd.unwrap_or(PathBuf::from(""));

    cwd.join("domains.txt")
}

fn main() {
    let f = get_file();

    if !f.exists() {
        eprintln!("Could not found the file specified");

        return;
    }

    let mut workers: Vec<JoinHandle<()>> = Vec::new();

    let num_of_cpus = available_parallelism().unwrap();

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let data: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));

    let finished = Arc::new(AtomicBool::new(false));

    {
        let _lines = lines.clone();

        let _finished = finished.clone();

        thread::spawn(move || {
            let content = read_to_string(f).unwrap_or(String::from(""));

            let lines = content.lines();

            let mut vec = _lines.lock().unwrap();

            for line in lines {
                vec.push(String::from(line));
            }

            _finished.store(true, Ordering::SeqCst);
        });
    }

    for _ in 0 .. num_of_cpus.get() {
        let _lines = lines.clone();

        let _finished = finished.clone();

        let _data = data.clone();

        let handle = thread::spawn(move || {
            while !_finished.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(100));
            }

            loop {
                let mut lines = _lines.lock().unwrap();

                if lines.is_empty() {
                    break;
                }

                let domain = lines.pop().unwrap();

                drop(lines);

                let expiration = SslExpiration::from_domain_name(&domain).unwrap();

                let result = if expiration.is_expired() {
                    vec![domain, String::from("Expired"), String::from("Expired")]
                } else {
                    let days = expiration.days();

                    let date = Utc::now();

                    let date = date.add(TimeDelta::days(days as i64));

                    let date = date.naive_local();

                    let date = format!("{}", date.format("%Y-%m-%d"));

                    vec![domain, days.to_string(), date]
                };

                let mut data = _data.lock().unwrap();

                data.push(result);
            }
        });

        workers.push(handle);

    }

    let handle: JoinHandle<bool> = thread::spawn(move || {
        for handle in workers {
            handle.join().unwrap();
        }

        let mut rows = data.lock().unwrap();

        rows.sort_by(|a, b| {
            let lhs = a.get(1).unwrap();
            let rhs = b.get(1).unwrap();

            let lhs = lhs.parse::<i32>().unwrap();
            let rhs = rhs.parse::<i32>().unwrap();

            lhs.cmp(&rhs)
        });

        let mut table = vec![];

        for row in rows.iter() {
            if row.len() < 3 {
                continue;
            }

            let first = row.first().unwrap();
            let second = row.get(1).unwrap();
            let third = row.get(2).unwrap();

            table.push(
                vec![
                    first.cell(),
                    second.cell().justify(Justify::Right),
                    third.cell().justify(Justify::Right)
                ]
            )
        }

        let table = table.table().title(vec![
            "Domain".cell(),
            "Expire Days".cell().justify(Justify::Right),
            "Expire Date".cell().justify(Justify::Right)
        ]).bold(true);

        print_stdout(table).is_ok()
    });

    let result = handle.join().unwrap();

    let code = match result {
        true => 0,
        false => -1
    };

    exit(code);
}
