use shredf::file;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "shredf",after_help=AFTER_HELP )]

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

static AFTER_HELP: &str =
    "Delete FILE(s) if --remove (-r) is specified.  The default is not to remove\n\
     the files because it is common to operate on device files like /dev/hda,\n\
     and those files usually should not be removed.\n\
     \n\
     CAUTION: Note that shred relies on a very important assumption:\n\
     that the file system overwrites data in place.  This is the traditional\n\
     way to do things, but many modern file system designs do not satisfy this\n\
     assumption.  The following are examples of file systems on which shred is\n\
     not effective, or is not guaranteed to be effective in all file system modes:\n\
     \n\
     * log-structured or journal file systems, such as those supplied with\n\
     AIX and Solaris (and JFS, ReiserFS, XFS, Ext3, etc.)\n\
     \n\
     * file systems that write redundant data and carry on even if some writes\n\
     fail, such as RAID-based file systems\n\
     \n\
     * file systems that make snapshots, such as Network Appliance's NFS server\n\
     \n\
     * file systems that cache in temporary locations, such as NFS\n\
     version 3 clients\n\
     \n\
     * compressed file systems\n\
     \n\
     In the case of ext3 file systems, the above disclaimer applies\n\
     and shred is thus of limited effectiveness) only in data=journal mode,\n\
     which journals file data in addition to just metadata.  In both the\n\
     data=ordered (default) and data=writeback modes, shred works as usual.\n\
     Ext3 journal modes can be changed by adding the data=something option\n\
     to the mount options for a particular file system in the /etc/fstab file,\n\
     as documented in the mount man page (man mount).\n\
     \n\
     In addition, file system backups and remote mirrors may contain copies\n\
     of the file that cannot be removed, and that will allow a shredded file\n\
     to be recovered later.\n\
     ";
