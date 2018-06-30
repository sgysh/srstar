extern crate srstar;

use srstar::Archiver;
use std::fs::File;

fn main() {
    let file = File::create("foo.tar").unwrap();
    let mut a = Archiver::new(file);

    a.add_file("README.md").unwrap();
    a.add_file("Cargo.toml").unwrap();
}
