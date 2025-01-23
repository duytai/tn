use std::fs;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::{Result, anyhow};
use pyo3::prelude::*;
use pyo3::types::*;
use pyo3::Python;
use std::ffi::CString;
use libc::{fork, waitpid, getpid};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Path to YAML file
    yaml_file: Option<String>,
    #[clap(subcommand)]
    command: Option<Command>,
    #[arg(short, long, default_value = "1")]
    /// Number of processes
    n_process: usize,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Creating a new project
    Init,
}

pub fn search_tn_file(searching_dir: PathBuf) -> Option<PathBuf> {
    let file_name = ".tn.yaml";
    if searching_dir.join(&file_name).exists() {
        Some(searching_dir.join(file_name))
    } else {
        let mut parent_directory = searching_dir;
        for _ in 0..parent_directory.iter().count() {
            parent_directory.pop();
            if parent_directory.join(&file_name).exists() {
                return Some(parent_directory.join(&file_name))
            }
        }
        None
    }
}

fn visit_config(yaml_file: String, project_dir: String, n_process: usize) -> Result<()>{
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR environment variable is not set");
    let script_file = PathBuf::from(&manifest_dir).join("src/script.py");
    let py_code = fs::read_to_string(&script_file)?;
    pyo3::prepare_freethreaded_python();

    let mut tasks = Python::with_gil(|py| -> Result<Vec<String>> {
        let globals = PyDict::new(py);
        let py_code = CString::new(py_code.clone())?;
        py.run(py_code.as_c_str(), Some(&globals), Some(&globals))?;
        if let Some(fn_sweep) = globals.get_item("sweep")? {
            let yaml_file = PyString::new(py, &yaml_file);
            let project_dir = PyString::new(py, &project_dir);
            let args = PyTuple::new(py, &[yaml_file, project_dir])?;
            let output: Vec<String> = fn_sweep.call1(args)?.extract()?;
            return Ok(output)
        }
        Ok(vec![])
    })?;

    println!("    number of tasks: {}", tasks.len());
    println!("number of processes: {}", n_process);

    let mut active_processes = 0;
    while active_processes > 0 || !tasks.is_empty() {
        while active_processes < n_process && !tasks.is_empty() {
            if let Some(task) = tasks.pop() {
                unsafe {
                    let pid = fork();
                    if pid == 0 {
                        return Python::with_gil(|py| -> Result<()> {
                            let globals = PyDict::new(py);
                            let py_code = CString::new(py_code.clone())?;
                            py.run(py_code.as_c_str(), Some(&globals), Some(&globals))?;
                            if let Some(fn_execute) = globals.get_item("execute")? {
                                let task = PyString::new(py, &task);
                                let args = PyTuple::new(py, &[task])?;
                                fn_execute.call1(args)?;
                                return Ok(())
                            }
                            Ok(())
                        })
                    } else if pid > 0 {
                        active_processes += 1;
                    } else {
                        eprintln!("fork failed!");
                    }
                }
            }
        }
        unsafe {
            let mut status = 0;
            let pid = waitpid(-1, &mut status, 0);
            if pid > 0 {
                active_processes -= 1;
            }
        }
    }

    unsafe {
        println!("all tasks completed, parent process (PID: {}) exiting.", getpid());
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = CliArgs::parse();
    let current_dir = std::env::current_dir()?;
    let tn_file = search_tn_file(current_dir.clone());

    if let Some(yaml_file) = args.yaml_file {
        return if let Some(f) = tn_file {
            let project_dir = f.parent()
                .and_then(|p| p.to_str())
                .and_then(|p| Some(p.to_string()))
                .unwrap();
            visit_config(yaml_file, project_dir, args.n_process)
        } else {
            Err(anyhow!("not a tn repository (or any of the parent directories): .tn.yaml"))
        }
    }

    if let Some(command) = args.command {
        return match command {
            Command::Init => {
                match tn_file {
                    Some(f) => Err(anyhow!("already initialized: {}", f.display())),
                    None => {
                        let f = current_dir.join(".tn.yaml");
                        fs::File::create(&f)?;
                        println!("created tn file: {}", f.display());
                        Ok(())
                    }
                }
            }
        }
    }
    Ok(())
}
