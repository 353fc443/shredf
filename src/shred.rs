// * This file is part of the uutils coreutils package.
// *
// * (c) Michael Rosenberg <42micro@gmail.com>
// * (c) Fort <forticulous@gmail.com>
// *
// * For the full copyright and license information, please view the LICENSE
// * file that was distributed with this source code.

// spell-checker:ignore (words) writeback wipesync

use rand::{Rng, ThreadRng};
use std::cell::{Cell, RefCell};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

macro_rules! executable(
    () => ({
        let module = module_path!();
        let module = module.split("::").next().unwrap_or(module);
        if &module[0..3] == "uu_" {
            &module[3..]
        } else {
            module
        }
    })
);

macro_rules! show_error(
    ($($args:tt)+) => ({
        eprint!("{}: ", executable!());
        eprintln!($($args)+);
    })
);

static NAME: &str = "shredf";
const BLOCK_SIZE: usize = 512;
const NAME_CHARSET: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_.";

// Patterns as shown in the GNU coreutils shred implementation
const PATTERNS: [&[u8]; 22] = [
    b"\x00",
    b"\xFF",
    b"\x55",
    b"\xAA",
    b"\x24\x92\x49",
    b"\x49\x24\x92",
    b"\x6D\xB6\xDB",
    b"\x92\x49\x24",
    b"\xB6\xDB\x6D",
    b"\xDB\x6D\xB6",
    b"\x11",
    b"\x22",
    b"\x33",
    b"\x44",
    b"\x66",
    b"\x77",
    b"\x88",
    b"\x99",
    b"\xBB",
    b"\xCC",
    b"\xDD",
    b"\xEE",
];

#[derive(Clone, Copy)]
enum PassType<'a> {
    Pattern(&'a [u8]),
    Random,
}

// Used to generate all possible filenames of a certain length using NAME_CHARSET as an alphabet
struct FilenameGenerator {
    name_len: usize,
    name_charset_indices: RefCell<Vec<usize>>, // Store the indices of the letters of our filename in NAME_CHARSET
    exhausted: Cell<bool>,
}

impl FilenameGenerator {
    fn new(name_len: usize) -> FilenameGenerator {
        let indices: Vec<usize> = vec![0; name_len];
        FilenameGenerator {
            name_len,
            name_charset_indices: RefCell::new(indices),
            exhausted: Cell::new(false),
        }
    }
}

impl Iterator for FilenameGenerator {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if self.exhausted.get() {
            return None;
        }

        let mut name_charset_indices = self.name_charset_indices.borrow_mut();

        // Make the return value, then increment
        let mut ret = String::new();
        for i in name_charset_indices.iter() {
            let c: char = NAME_CHARSET.chars().nth(*i).unwrap();
            ret.push(c);
        }

        if name_charset_indices[0] == NAME_CHARSET.len() - 1 {
            self.exhausted.set(true)
        }
        // Now increment the least significant index
        for i in (0..self.name_len).rev() {
            if name_charset_indices[i] == NAME_CHARSET.len() - 1 {
                name_charset_indices[i] = 0; // Carry the 1
                continue;
            } else {
                name_charset_indices[i] += 1;
                break;
            }
        }

        Some(ret)
    }
}

// Used to generate blocks of bytes of size <= BLOCK_SIZE based on either a give pattern
// or randomness
struct BytesGenerator<'a> {
    total_bytes: u64,
    bytes_generated: Cell<u64>,
    block_size: usize,
    exact: bool, // if false, every block's size is block_size
    gen_type: PassType<'a>,
    rng: Option<RefCell<ThreadRng>>,
    bytes: [u8; BLOCK_SIZE],
}

impl<'a> BytesGenerator<'a> {
    fn new(total_bytes: u64, gen_type: PassType<'a>, exact: bool) -> BytesGenerator {
        let rng = match gen_type {
            PassType::Random => Some(RefCell::new(rand::thread_rng())),
            _ => None,
        };

        let bytes = [0; BLOCK_SIZE];

        BytesGenerator {
            total_bytes,
            bytes_generated: Cell::new(0u64),
            block_size: BLOCK_SIZE,
            exact,
            gen_type,
            rng,
            bytes,
        }
    }

    pub fn reset(&mut self, total_bytes: u64, gen_type: PassType<'a>) {
        if let PassType::Random = gen_type {
            if self.rng.is_none() {
                self.rng = Some(RefCell::new(rand::thread_rng()));
            }
        }

        self.total_bytes = total_bytes;
        self.gen_type = gen_type;

        self.bytes_generated.set(0);
    }

    pub fn next(&mut self) -> Option<&[u8]> {
        // We go over the total_bytes limit when !self.exact and total_bytes isn't a multiple
        // of self.block_size
        if self.bytes_generated.get() >= self.total_bytes {
            return None;
        }

        let this_block_size = {
            if !self.exact {
                self.block_size
            } else {
                let bytes_left = self.total_bytes - self.bytes_generated.get();
                if bytes_left >= self.block_size as u64 {
                    self.block_size
                } else {
                    (bytes_left % self.block_size as u64) as usize
                }
            }
        };

        let bytes = &mut self.bytes[..this_block_size];

        match self.gen_type {
            PassType::Random => {
                let mut rng = self.rng.as_ref().unwrap().borrow_mut();
                rng.fill(bytes);
            }
            PassType::Pattern(pattern) => {
                let skip = {
                    if self.bytes_generated.get() == 0 {
                        0
                    } else {
                        (pattern.len() as u64 % self.bytes_generated.get()) as usize
                    }
                };

                // Copy the pattern in chunks rather than simply one byte at a time
                let mut i = 0;
                while i < this_block_size {
                    let start = (i + skip) % pattern.len();
                    let end = (this_block_size - i).min(pattern.len());
                    let len = end - start;

                    bytes[i..i + len].copy_from_slice(&pattern[start..end]);

                    i += len;
                }
            }
        };

        let new_bytes_generated = self.bytes_generated.get() + this_block_size as u64;
        self.bytes_generated.set(new_bytes_generated);

        Some(bytes)
    }
}

pub mod options {
    pub const FORCE: &str = "force";
    pub const FILE: &str = "file";
    pub const ITERATIONS: &str = "iterations";
    pub const SIZE: &str = "size";
    pub const REMOVE: &str = "remove";
    pub const VERBOSE: &str = "verbose";
    pub const EXACT: &str = "exact";
    pub const ZERO: &str = "zero";
}

fn pass_name(pass_type: PassType) -> String {
    match pass_type {
        PassType::Random => String::from("random"),
        PassType::Pattern(bytes) => {
            let mut s: String = String::new();
            while s.len() < 6 {
                for b in bytes {
                    let readable: String = format!("{:x}", b);
                    s.push_str(&readable);
                }
            }
            s
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn wipe_file(
    path_str: &str,
    n_passes: usize,
    remove: bool,
    size: Option<u64>,
    exact: bool,
    zero: bool,
    verbose: bool,
    force: bool,
) {
    // Get these potential errors out of the way first
    let path: &Path = Path::new(path_str);
    if !path.exists() {
        show_error!("{}: No such file or directory", path.display());
        return;
    }
    if !path.is_file() {
        show_error!("{}: Not a file", path.display());
        return;
    }

    // If force is true, set file permissions to not-readonly.
    if force {
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                show_error!("{}", e);
                return;
            }
        };

        let mut perms = metadata.permissions();
        perms.set_readonly(false);
        if let Err(e) = fs::set_permissions(path, perms) {
            show_error!("{}", e);
            return;
        }
    }

    // Fill up our pass sequence
    let mut pass_sequence: Vec<PassType> = Vec::new();

    if n_passes <= 3 {
        // Only random passes if n_passes <= 3
        for _ in 0..n_passes {
            pass_sequence.push(PassType::Random)
        }
    }
    // First fill it with Patterns, shuffle it, then evenly distribute Random
    else {
        let n_full_arrays = n_passes / PATTERNS.len(); // How many times can we go through all the patterns?
        let remainder = n_passes % PATTERNS.len(); // How many do we get through on our last time through?

        for _ in 0..n_full_arrays {
            for p in &PATTERNS {
                pass_sequence.push(PassType::Pattern(*p));
            }
        }
        for pattern in PATTERNS.iter().take(remainder) {
            pass_sequence.push(PassType::Pattern(pattern));
        }
        rand::thread_rng().shuffle(&mut pass_sequence[..]); // randomize the order of application

        let n_random = 3 + n_passes / 10; // Minimum 3 random passes; ratio of 10 after
                                          // Evenly space random passes; ensures one at the beginning and end
        for i in 0..n_random {
            pass_sequence[i * (n_passes - 1) / (n_random - 1)] = PassType::Random;
        }
    }

    // --zero specifies whether we want one final pass of 0x00 on our file
    if zero {
        pass_sequence.push(PassType::Pattern(b"\x00"));
    }

    {
        let total_passes: usize = pass_sequence.len();
        let mut file: File = match OpenOptions::new().write(true).truncate(false).open(path) {
            Ok(f) => f,
            Err(e) => {
                show_error!("{}: failed to open for writing: {}", path.display(), e);
                return;
            }
        };

        // NOTE: it does not really matter what we set for total_bytes and gen_type here, so just
        //       use bogus values
        let mut generator = BytesGenerator::new(0, PassType::Pattern(&[]), exact);

        for (i, pass_type) in pass_sequence.iter().enumerate() {
            if verbose {
                let pass_name: String = pass_name(*pass_type);
                if total_passes.to_string().len() == 1 {
                    println!(
                        "{}: {}: pass {}/{} ({})... ",
                        NAME,
                        path.display(),
                        i + 1,
                        total_passes,
                        pass_name
                    );
                } else {
                    println!(
                        "{}: {}: pass {:2.0}/{:2.0} ({})... ",
                        NAME,
                        path.display(),
                        i + 1,
                        total_passes,
                        pass_name
                    );
                }
            }
            // size is an optional argument for exactly how many bytes we want to shred
            match do_pass(&mut file, path, &mut generator, *pass_type, size) {
                Ok(_) => {}
                Err(e) => {
                    show_error!("{}: File write pass failed: {}", path.display(), e);
                }
            }
            // Ignore failed writes; just keep trying
        }
    }

    if remove {
        match do_remove(path, path_str, verbose) {
            Ok(_) => {}
            Err(e) => {
                show_error!("{}: failed to remove file: {}", path.display(), e);
            }
        }
    }
}

fn do_pass<'a>(
    file: &mut File,
    path: &Path,
    generator: &mut BytesGenerator<'a>,
    generator_type: PassType<'a>,
    given_file_size: Option<u64>,
) -> Result<(), io::Error> {
    file.seek(SeekFrom::Start(0))?;

    // Use the given size or the whole file if not specified
    let size: u64 = given_file_size.unwrap_or(get_file_size(path)?);

    generator.reset(size, generator_type);

    while let Some(block) = generator.next() {
        file.write_all(block)?;
    }

    file.sync_data()?;

    Ok(())
}

fn get_file_size(path: &Path) -> Result<u64, io::Error> {
    let size: u64 = fs::metadata(path)?.len();

    Ok(size)
}

// Repeatedly renames the file with strings of decreasing length (most likely all 0s)
// Return the path of the file after its last renaming or None if error
fn wipe_name(orig_path: &Path, verbose: bool) -> Option<PathBuf> {
    let file_name_len: usize = orig_path.file_name().unwrap().to_str().unwrap().len();

    let mut last_path: PathBuf = PathBuf::from(orig_path);

    for length in (1..=file_name_len).rev() {
        for name in FilenameGenerator::new(length) {
            let new_path: PathBuf = orig_path.with_file_name(name);
            // We don't want the filename to already exist (don't overwrite)
            // If it does, find another name that doesn't
            if new_path.exists() {
                continue;
            }
            match fs::rename(&last_path, &new_path) {
                Ok(()) => {
                    if verbose {
                        println!(
                            "{}: {}: renamed to {}",
                            NAME,
                            last_path.display(),
                            new_path.display()
                        );
                    }

                    // Sync every file rename
                    {
                        let new_file: File = File::open(new_path.clone())
                            .expect("Failed to open renamed file for syncing");
                        new_file.sync_all().expect("Failed to sync renamed file");
                    }

                    last_path = new_path;
                    break;
                }
                Err(e) => {
                    println!(
                        "{}: {}: Couldn't rename to {}: {}",
                        NAME,
                        last_path.display(),
                        new_path.display(),
                        e
                    );
                    return None;
                }
            }
        } // If every possible filename already exists, just reduce the length and try again
    }

    Some(last_path)
}

fn do_remove(path: &Path, orig_filename: &str, verbose: bool) -> Result<(), io::Error> {
    if verbose {
        println!("{}: {}: removing", NAME, orig_filename);
    }

    let renamed_path: Option<PathBuf> = wipe_name(path, verbose);
    if let Some(rp) = renamed_path {
        fs::remove_file(rp)?;
    }

    if verbose {
        println!("{}: {}: removed", NAME, orig_filename);
    }

    Ok(())
}
