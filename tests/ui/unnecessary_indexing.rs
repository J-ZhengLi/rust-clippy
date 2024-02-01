#![warn(clippy::unnecessary_indexing)]

fn do_something() {}

fn main() {
    let arr = [1, 2, 3];

    if !arr.is_empty() {
        let x = arr[0];
    }
    if arr.is_empty() {
        do_something();
    } else {
        let x = arr[0];
    }

    // Don't lint
    if arr.is_empty() {
        let x = arr[0];
    }
    // Don't lint
    if !arr.is_empty() {
        let x = arr[0];
        let y = arr[1];
    }
}
