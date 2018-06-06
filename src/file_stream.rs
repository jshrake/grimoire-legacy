use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use error::{Error, Result};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

pub struct FileStream {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    watcher_rx: Receiver<DebouncedEvent>,
    path: PathBuf,
    force_load: bool,
}

impl FileStream {
    pub fn new(path: &Path) -> Result<FileStream> {
        let (watcher_tx, watcher_rx) = channel();
        let mut watcher: RecommendedWatcher =
            Watcher::new(watcher_tx, Duration::from_millis(200)).map_err(|err| Error::notify(err))?;
        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|err| Error::watch_path(path, err))?;
        Ok(FileStream {
            watcher: watcher,
            watcher_rx: watcher_rx,
            path: PathBuf::from(path),
            force_load: true,
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn try_recv(&mut self) -> Result<Option<Vec<u8>>> {
        let event = self.watcher_rx.try_recv();
        let should_read = match event {
            Ok(DebouncedEvent::Write(_)) | Ok(DebouncedEvent::Create(_)) => true,
            Ok(_) | Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                return Err(Error::bug("FileStream::try_recv got unexpected disconnect"));
            }
        };
        if self.force_load || should_read {
            self.force_load = false;
            let mut bytes = Vec::new();
            File::open(&self.path)
                .map_err(|err| Error::io(&self.path, err))?
                .read_to_end(&mut bytes)
                .map_err(|err| Error::io(&self.path, err))?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }
}
