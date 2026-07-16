//! Agent Simulation Kernel (ASK) — digital world kernel for agents.
//!
//! FrogComposband-inspired phases: collect → apply → world systems → commit view → tick++.
//! Thought inherits; code does not.

pub mod actions;
pub mod agent_view;
pub mod agents;
pub mod art;
pub mod auth;
pub mod balance;
pub mod components;
pub mod config;
pub mod describe;
pub mod events;
pub mod f_info;
pub mod gateway;
pub mod generate; // generate/mod.rs
pub mod grid;
pub mod inspect;
pub mod k_info;
pub mod memory;
pub mod org;
pub mod persist;
pub mod player;
pub mod r_info;
pub mod sandbox;
pub mod serve;
pub mod skill;
pub mod spatial;
pub mod systems;
pub mod tick;
pub mod vaults;
pub mod view;
pub mod viewer;
pub mod vision;
pub mod world;

pub use config::Config;
pub use tick::Sim;
pub use world::KernelWorld;
