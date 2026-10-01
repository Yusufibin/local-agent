#![cfg(windows)]
use deskpi_lib::sidecar::process_alive;
use deskpi_lib::windows_job::Job;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn closing_job_kills_the_sidecar_and_its_child() {
    let job = Job::new().unwrap();
    let mut child = Command::new("node")
        .args(["-e", "setTimeout(()=>{const c=require('child_process').spawn(process.execPath,['-e','setInterval(()=>{},1000)']); console.log(c.pid)},300); setInterval(()=>{},1000)"])
        .stdout(Stdio::piped()).spawn().unwrap();
    job.assign(&child).unwrap();
    let parent = child.id();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let descendant = line.trim().parse::<u32>().unwrap();
    assert!(process_alive(parent));
    assert!(process_alive(descendant));
    drop(job);
    let deadline = Instant::now() + Duration::from_secs(3);
    while (process_alive(parent) || process_alive(descendant)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!process_alive(parent));
    assert!(!process_alive(descendant));
    let _ = child.wait();
}
