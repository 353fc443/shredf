use shredf::file;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "shredf")]
struct Opt {
    #[structopt(short, long, required = true, help = "Folder to run shredf on")]
    folder: String,
    #[structopt(short, long, help = "Run on verbose mode")]
    verbose: bool,
    #[structopt(short, long, help = "Number of n passes", default_value = "3")]
    n_passes: usize,
    #[structopt(short, long, help = "Remove the files")]
    remove: bool,
    #[structopt(short, long, help = "Add 0x00 to our final pass")]
    zeroes: bool,
    #[structopt(
        short,
        long,
        help = "do not round file sizes up to the next full block;\n\
    this is the default for non-regular files"
    )]
    exact: bool,
    #[structopt(long, help = "change permissions to allow writing if necessary")]
    force: bool,
}

fn main() {
    let opt = Opt::from_args();
    for file in file::walk_dir(&opt.folder).unwrap() {
        file.wipe(
            opt.n_passes,
            opt.remove,
            opt.exact,
            opt.zeroes,
            opt.verbose,
            opt.force,
        );
    }
}
