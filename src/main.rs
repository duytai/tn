use std::fs;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::{Result, anyhow};
use pyo3::prelude::*;
use pyo3::types::*;
use pyo3::Python;
use std::ffi::CString;
use libc::{fork, waitpid};
use indicatif::{ProgressBar, ProgressStyle, ProgressState};
use std::{fmt::Write};
use std::collections::VecDeque;

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
    #[arg(short, long, default_value = "false")]
    /// Run sweep
    sweep: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Creating a new project
    Init,
}

pub fn search_tn_dir(searching_dir: PathBuf) -> Option<PathBuf> {
    let dirname = ".tn/";
    if searching_dir.join(&dirname).exists() {
        Some(searching_dir.join(dirname))
    } else {
        let mut parent_directory = searching_dir;
        for _ in 0..parent_directory.iter().count() {
            parent_directory.pop();
            if parent_directory.join(&dirname).exists() {
                return Some(parent_directory.join(&dirname))
            }
        }
        None
    }
}

fn visit_config(yaml_file: String, project_dir: String, n_process: usize, sweep_only: bool) -> Result<()>{
    let py_code = include_str!("script.py");
    pyo3::prepare_freethreaded_python();

    let tasks = Python::with_gil(|py| -> Result<Vec<String>> {
        let globals = PyDict::new(py);
        let py_code = CString::new(py_code)?;
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
    let mut tasks = tasks.into_iter().collect::<VecDeque<_>>();

    let bar= ProgressBar::new(tasks.len() as u64);
    bar.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));
    let mut active_processes = 0;
    while active_processes > 0 || !tasks.is_empty() {
        while active_processes < n_process && !tasks.is_empty() {
            if let Some(task) = tasks.pop_front() {
                unsafe {
                    let pid = fork();
                    if pid == 0 {
                        return Python::with_gil(|py| -> Result<()> {
                            let globals = PyDict::new(py);
                            let py_code = CString::new(py_code)?;
                            py.run(py_code.as_c_str(), Some(&globals), Some(&globals))?;
                            if let Some(fn_execute) = globals.get_item("execute")? {
                                let task = PyString::new(py, &task).to_object(py);
                                let sweep_only = PyBool::new(py, sweep_only).to_object(py);
                                let args = PyTuple::new(py, &[task, sweep_only])?;
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
                bar.inc(1);
                active_processes -= 1;
            }
        }
    }
    bar.finish_with_message("done");
    Ok(())
}

fn main() -> Result<()> {
    let args = CliArgs::parse();
    let current_dir = std::env::current_dir()?;
    let tn_dir = search_tn_dir(current_dir.clone());

    if let Some(yaml_file) = args.yaml_file {
        return if let Some(f) = tn_dir {
            let project_dir = f.parent()
                .and_then(|p| p.to_str())
                .and_then(|p| Some(p.to_string()))
                .unwrap();
            let yaml_file = format!("{}/{}", current_dir.display(), yaml_file);
            visit_config(yaml_file, project_dir, args.n_process, args.sweep)
        } else {
            Err(anyhow!("not a tn repository (or any of the parent directories): .tn/"))
        }
    }

    if let Some(command) = args.command {
        return match command {
            Command::Init => {
                match tn_dir {
                    Some(d) => Err(anyhow!("already initialized: {}", d.display())),
                    None => {
                        let tn_dir = current_dir.join(".tn/");
                        fs::create_dir_all(&tn_dir)?;
                        println!("created tn dir: {}", tn_dir.display());
                        Ok(())
                    }
                }
            }
        }
    }
    Ok(())
}
