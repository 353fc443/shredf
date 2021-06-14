use shredf::file;

fn main() {
    for file in file::walk_dir(".").unwrap() {
        println!("{}", file.name);
    }
}
