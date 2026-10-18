// SPDX-License-Identifier: MIT
mod audio;
mod engine;
mod groq;
mod model;
mod native;
mod shortcuts;
mod store;
mod text;

pub fn run() {
    engine::run();
}
