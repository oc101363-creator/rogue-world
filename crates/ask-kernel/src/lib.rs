//! Agent Simulation Kernel (ASK) — digital world kernel for agents.
//!
//! FrogComposband-inspired phases: collect → apply → world systems → commit view → tick++.
//! Thought inherits; code does not.

pub mod actions;
pub mod agents;
pub mod components;
pub mod config;
pub mod events;
pub mod gateway;
pub mod grid;
pub mod memory;
pub mod org;
pub mod persist;
pub mod skill;
pub mod systems;
pub mod tick;
pub mod view;
pub mod world;

pub use config::Config;
pub use tick::Sim;
pub use world::KernelWorld;
