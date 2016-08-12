extern crate fuse;
extern crate libc;
extern crate time;
extern crate hyper;
extern crate rustc_serialize;
extern crate clap;

use std::collections::BTreeMap;
use std::path::Path;
use std::env;
use std::io::prelude::Read;
use libc::{ENOENT, ENOSYS};
use time::Timespec;
use fuse::{FileAttr, FileType, Filesystem, Request, ReplyAttr, ReplyData, ReplyEntry, ReplyDirectory, ReplyOpen};
use hyper::client::{Client, Response};
use rustc_serialize::json::Json;
use clap::{App, Arg};

struct TwitchFileSystem {
    attrs: BTreeMap<u64, FileAttr>,
    inodes: BTreeMap<String, u64>
}

impl TwitchFileSystem {
    fn new() -> TwitchFileSystem {
        let mut attrs = BTreeMap::new();
        let mut inodes = BTreeMap::new();
        let ts = time::now().to_timespec();
        let attr = FileAttr {
            ino: 1,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0
        };
        attrs.insert(1, attr);
        inodes.insert("/".to_owned(), 1);
        let mut body = String::new();
        Client::new()
            .get("https://api.twitch.tv/kraken/streams/featured")
            .send()
            .expect("Couldn't load twitch")
            .read_to_string(&mut body);

        match Json::from_str(&body) {
            Ok(data) => {
                let streams = data.find("featured").unwrap().as_array().unwrap();
                for (i, stream) in streams.iter().enumerate() {
                    let attr = FileAttr {
                        ino: i as u64 + 2,
                        size: 0,
                        blocks: 0,
                        atime: ts,
                        mtime: ts,
                        ctime: ts,
                        crtime: ts,
                        kind: FileType::RegularFile,
                        perm: 0o644,
                        nlink: 0,
                        uid: 0,
                        gid: 0,
                        rdev: 0,
                        flags: 0
                    };

                    let game = stream
                        .find_path(&["stream", "game"])
                        .unwrap()
                        .as_string()
                        .unwrap();

                    let name = stream
                        .find_path(&["stream", "channel", "display_name"])
                        .unwrap()
                        .as_string()
                        .unwrap();

                    attrs.insert(attr.ino, attr);
                    inodes.insert(format!("{} - {}", name, game), attr.ino);
                }
            },
            Err(_) => println!("Twitch returned invalid json")
        }
        TwitchFileSystem {attrs: attrs, inodes: inodes}
    }
}

impl Filesystem for TwitchFileSystem {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match self.attrs.get(&ino) {
            Some(attr) => {
                let ttl = Timespec::new(1, 0);
                reply.attr(&ttl, attr);
            },
            None => reply.error(ENOENT)
        }
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &Path, reply: ReplyEntry) {
        let inode = match self.inodes.get(name.to_str().unwrap()) {
            Some(inode) => inode,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        match self.attrs.get(inode) {
            Some(attr) => {
                let ttl = Timespec::new(1, 0);
                reply.entry(&ttl, attr, 0);
            },
            None => reply.error(ENOENT),
        };
    }

    fn readdir(&mut self, _req: &Request, ino: u64, fh: u64, offset: u64, mut reply: ReplyDirectory) {
        if offset == 0 {
            if ino == 1 {
                for (stream, &inode) in &self.inodes {
                    if inode == 1 { continue; }
                    let offset = inode;
                    reply.add(inode, offset, FileType::RegularFile, &Path::new(stream));
                }
                reply.add(1, 0, FileType::Directory, &Path::new("."));
                reply.add(1, 1, FileType::Directory, &Path::new(".."));
            }
        }

        reply.ok();
    }
    fn read(&mut self, _req: &Request, ino: u64, fh: u64, offset: u64, size: u32, reply: ReplyData) {
        reply.data("test".as_bytes());
    }
}

fn is_valid_dir(mountpoint: String) -> Result<(), String> {
    match Path::new(&mountpoint).is_dir() {
        true => Ok(()),
        false => Err("Mountpoint must be a directory".to_string()),
    }
}

fn main() {
    let matches = App::new("twitch-fs")
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("unknown version"))
        .arg(Arg::with_name("mountpoint")
             .validator(is_valid_dir)
             .index(1)
             .required(true))
        .get_matches();

    // unwrap() is safe here because the argument is set as required
    let mountpoint = matches.value_of_os("mountpoint").unwrap();
    let fs = TwitchFileSystem::new();

    fuse::mount(fs , &mountpoint, &[])
}
