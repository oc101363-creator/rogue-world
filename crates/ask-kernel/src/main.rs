//! ASK Kernel CLI

use std::env;
use std::process;
use std::thread;
use std::time::Duration;

use ask_kernel::config::Config;
use ask_kernel::persist;
use ask_kernel::tick::Sim;
use ask_kernel::view;
use ask_kernel::world::KernelWorld;
use ask_kernel::events::EventBuf;

fn usage() -> ! {
    eprintln!(
        "ask-kernel — Agent Simulation Kernel

Usage:
  ask-kernel [--steps N] [--watch] [--tick-ms MS] [--save PATH] [--load PATH] [--seed N]

Examples:
  ask-kernel --steps 40
  ask-kernel --watch --tick-ms 200
  ask-kernel --steps 20 --save data/world.json
  ask-kernel --load data/world.json --steps 10
"
    );
    process::exit(1);
}

fn main() {
    let mut steps: Option<u64> = None;
    let mut watch = false;
    let mut tick_ms: u64 = 200;
    let mut save_path: Option<String> = None;
    let mut load_path: Option<String> = None;
    let mut seed: u64 = 1;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => usage(),
            "--steps" => {
                i += 1;
                steps = Some(args.get(i).and_then(|s| s.parse().ok()).unwrap_or(30));
            }
            "--watch" => watch = true,
            "--tick-ms" => {
                i += 1;
                tick_ms = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(200);
            }
            "--save" => {
                i += 1;
                save_path = args.get(i).cloned();
            }
            "--load" => {
                i += 1;
                load_path = args.get(i).cloned();
            }
            "--seed" => {
                i += 1;
                seed = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1);
            }
            other => {
                eprintln!("unknown arg: {other}");
                usage();
            }
        }
        i += 1;
    }

    let mut cfg = Config::default();
    cfg.seed = seed;

    let kernel = if let Some(path) = &load_path {
        match persist::load_from_path(path) {
            Ok(k) => {
                eprintln!("loaded {path}");
                k
            }
            Err(e) => {
                eprintln!("load failed: {e:#}");
                process::exit(2);
            }
        }
    } else {
        KernelWorld::new(&cfg)
    };

    let mut sim = Sim::new(kernel);

    if watch {
        eprintln!("watch mode — Ctrl+C to stop");
        loop {
            sim.step();
            // clear-ish
            print!("\x1B[2J\x1B[H");
            print!("{}", view::render(&mut sim.kernel.world));
            let ev = sim.kernel.world.resource_mut::<EventBuf>().drain();
            for e in ev.iter().take(8) {
                println!("  {e:?}");
            }
            thread::sleep(Duration::from_millis(tick_ms));
        }
    } else {
        let n = steps.unwrap_or(30);
        sim.run_steps(n, true);
        if let Some(path) = save_path {
            if let Err(e) = persist::save_to_path(&mut sim.kernel.world, &path) {
                eprintln!("save failed: {e:#}");
                process::exit(3);
            }
            eprintln!("saved {path}");
        }
    }
}
