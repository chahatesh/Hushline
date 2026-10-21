// SPDX-License-Identifier: MIT
mod audio;
mod deadline;
mod engine;
mod groq;
mod lifecycle;
mod model;
mod native;
mod offline;
mod recovery;
mod shortcuts;
mod store;
mod text;

pub fn run() {
    engine::run();
}
