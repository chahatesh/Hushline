// SPDX-License-Identifier: MIT
mod audio;
mod deadline;
mod engine;
mod groq;
mod insights;
mod lifecycle;
mod model;
mod native;
mod offline;
mod prompt;
mod quick;
mod recovery;
mod shortcuts;
mod store;
mod text;

pub fn run() {
    engine::run();
}
