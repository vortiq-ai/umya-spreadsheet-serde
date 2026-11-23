use std::{
    fs::File,
    io,
    io::Read,
    path::{
        Path,
        PathBuf,
    },
};

use crate::{
    XlsxError,
    reader::driver::{
        join_paths,
        zip_by_name,
    },
    structs::{
        StringValue,
        WriterManager,
    },
};

/// A whole package held in memory, which a part can be read from again when it
/// is needed: the buffer `reader::xlsx::read_shared` was given. Shared, never
/// copied.
#[derive(Clone)]
pub(crate) struct SharedPackage(std::sync::Arc<dyn AsRef<[u8]> + Send + Sync>);

impl SharedPackage {
    pub(crate) fn new<T: AsRef<[u8]> + Send + Sync + 'static>(package: std::sync::Arc<T>) -> Self {
        Self(package)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        (*self.0).as_ref()
    }
}

impl std::fmt::Debug for SharedPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SharedPackage({} bytes)", self.bytes().len())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub(crate) struct RawFile {
    file_target:    StringValue,
    file_data:      Vec<u8>,
    source_file:    Option<PathBuf>,
    /// The in-memory package this part is read from when it is needed, when it
    /// was not read at open (`set_attributes_deferred`). Never serialized: it
    /// is a handle, not content, so a part still deferred is written without
    /// its bytes. Serialization stays derived (the consumers' serde schema
    /// audit admits no manual `Serialize`): read the sheets of a book read with
    /// `read_shared`, or load its parts, before serializing it.
    #[cfg_attr(feature = "serde", serde(skip))]
    source_package: Option<SharedPackage>,
}

impl RawFile {
    #[inline]
    pub(crate) fn file_name(&self) -> &str {
        self.file_target().split('/').next_back().unwrap()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use file_name()")]
    pub(crate) fn get_file_name(&self) -> &str {
        self.file_name()
    }

    #[inline]
    pub(crate) fn make_rel_name(&self) -> String {
        format!("_rels/{}.rels", self.file_name())
    }

    #[inline]
    pub(crate) fn path(&self) -> String {
        let mut v: Vec<&str> = self.file_target().split('/').collect();
        v.pop();
        v.join("/")
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use path()")]
    pub(crate) fn get_path(&self) -> String {
        self.path()
    }

    #[inline]
    pub(crate) fn extension(&self) -> String {
        self.file_name()
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_lowercase())
            .unwrap()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use extension()")]
    pub(crate) fn get_extension(&self) -> String {
        self.extension()
    }

    #[inline]
    pub(crate) fn file_target(&self) -> &str {
        self.file_target.value_str()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use file_target()")]
    pub(crate) fn get_file_target(&self) -> &str {
        self.file_target()
    }

    #[inline]
    pub(crate) fn set_file_target<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.file_target.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn file_data(&self) -> &[u8] {
        &self.file_data
    }

    #[inline]
    pub(crate) fn has_file_data(&self) -> bool {
        !self.file_data.is_empty()
    }

    #[inline]
    pub(crate) fn source_file(&self) -> Option<&Path> {
        self.source_file.as_deref()
    }

    /// The in-memory package this part is still to be read from, if it was
    /// deferred.
    #[inline]
    pub(crate) fn source_package(&self) -> Option<&[u8]> {
        self.source_package.as_ref().map(SharedPackage::bytes)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use file_data()")]
    pub(crate) fn get_file_data(&self) -> &[u8] {
        self.file_data()
    }

    #[inline]
    pub(crate) fn file_data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.file_data
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use file_data_mut()")]
    pub(crate) fn get_file_data_mut(&mut self) -> &mut Vec<u8> {
        self.file_data_mut()
    }

    #[inline]
    pub(crate) fn set_file_data(&mut self, value: &[u8]) -> &mut Self {
        self.file_data = value.into();
        self
    }

    #[inline]
    pub(crate) fn set_source_file<P: AsRef<Path>>(&mut self, value: P) -> &mut Self {
        self.source_file = Some(value.as_ref().to_path_buf());
        self
    }

    pub(crate) fn take_file_data(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.file_data)
    }

    pub(crate) fn source_file_owned(&self) -> Option<PathBuf> {
        self.source_file.clone()
    }

    pub(crate) fn read_from_source_or_empty(target: &str, source_file: Option<&Path>) -> Vec<u8> {
        let Some(source_file) = source_file else {
            return Vec::new();
        };
        let Ok(file) = File::open(source_file) else {
            return Vec::new();
        };
        let Ok(mut archive) = zip::read::ZipArchive::new(file) else {
            return Vec::new();
        };
        let Ok(mut source) = zip_by_name(&mut archive, target) else {
            return Vec::new();
        };
        let mut data = Vec::new();
        if source.read_to_end(&mut data).is_err() {
            data.clear();
        }
        data
    }

    pub(crate) fn set_attributes<R: Read + io::Seek>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        base_path: &str,
        target: &str,
    ) {
        let path_str = join_paths(base_path, target);
        let Ok(file) = zip_by_name(arv, &path_str) else {
            // File not found in archive — skip gracefully.
            self.set_file_target(path_str);
            return;
        };
        let mut r = io::BufReader::new(file);
        let mut buf = Vec::new();
        if r.read_to_end(&mut buf).is_err() {
            self.set_file_target(path_str);
            return;
        }

        self.set_file_target(path_str);
        self.file_data = buf;
    }

    /// Name the part without reading it: it is read from `package` when it is
    /// needed. A sheet's XML is most of a large package, and reading every
    /// sheet's at open held all of them at once (872 MB on a 13-sheet, 477
    /// MB-of-XML workbook) until each sheet was read. A part the
    /// package does not hold is named alone, as `set_attributes` does.
    pub(crate) fn set_attributes_deferred<R: Read + io::Seek>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        base_path: &str,
        target: &str,
        package: SharedPackage,
    ) {
        let path_str = join_paths(base_path, target);
        let present = zip_by_name(arv, &path_str).is_ok();
        self.set_file_target(path_str);
        if present {
            self.source_package = Some(package);
        }
    }

    pub(crate) fn set_attributes_from_source<R: Read + io::Seek, P: AsRef<Path>>(
        &mut self,
        arv: &mut zip::read::ZipArchive<R>,
        base_path: &str,
        target: &str,
        source_file: P,
    ) {
        self.set_attributes(arv, base_path, target);
        self.set_source_file(source_file);
    }

    pub(crate) fn load_file_data_from_source(&mut self) -> Result<(), XlsxError> {
        if !self.file_data.is_empty() || self.file_target().is_empty() {
            return Ok(());
        }
        self.file_data = self.read_data()?;
        Ok(())
    }

    /// Whether the part's bytes contain `needle`. A part not in memory is
    /// searched as it is decompressed, a chunk at a time, so a large sheet
    /// is not held whole to be searched.
    pub(crate) fn contains_bytes(&self, needle: &[u8]) -> Result<bool, XlsxError> {
        fn found(haystack: &[u8], needle: &[u8]) -> bool {
            !needle.is_empty()
                && haystack
                    .windows(needle.len())
                    .any(|window| window == needle)
        }
        fn search(mut source: impl Read, needle: &[u8]) -> Result<bool, XlsxError> {
            let keep = needle.len().saturating_sub(1);
            let mut window = Vec::with_capacity(64 * 1024 + keep);
            let mut chunk = vec![0u8; 64 * 1024];
            loop {
                let read = source.read(&mut chunk)?;
                if read == 0 {
                    return Ok(false);
                }
                window.extend_from_slice(&chunk[..read]);
                if found(&window, needle) {
                    return Ok(true);
                }
                let tail = window.len().saturating_sub(keep);
                window.drain(..tail);
            }
        }
        if !self.file_data.is_empty() {
            return Ok(found(&self.file_data, needle));
        }
        if let Some(source_file) = self.source_file.as_ref() {
            let mut archive = zip::read::ZipArchive::new(File::open(source_file)?)?;
            return search(zip_by_name(&mut archive, self.file_target())?, needle);
        }
        if let Some(package) = self.source_package() {
            let mut archive = zip::read::ZipArchive::new(io::Cursor::new(package))?;
            return search(zip_by_name(&mut archive, self.file_target())?, needle);
        }
        Ok(false)
    }

    /// The part's bytes, from memory or read from the source file or package.
    pub(crate) fn read_data(&self) -> Result<Vec<u8>, XlsxError> {
        if !self.file_data.is_empty() {
            return Ok(self.file_data.clone());
        }
        if self.source_file.is_none() {
            if let Some(package) = self.source_package() {
                let mut archive = zip::read::ZipArchive::new(io::Cursor::new(package))?;
                let mut source = zip_by_name(&mut archive, self.file_target())?;
                let mut buf = Vec::new();
                source.read_to_end(&mut buf)?;
                return Ok(buf);
            }
        }
        let Some(source_file) = self.source_file.as_ref() else {
            return Ok(Vec::new());
        };
        let mut archive = zip::read::ZipArchive::new(File::open(source_file)?)?;
        let mut source = zip_by_name(&mut archive, self.file_target())?;
        let mut buf = Vec::new();
        source.read_to_end(&mut buf)?;
        Ok(buf)
    }

    pub(crate) fn write_to_target<W: io::Seek + io::Write>(
        &self,
        target: &str,
        writer_mng: &mut WriterManager<W>,
    ) -> Result<(), XlsxError> {
        if !self.file_data().is_empty() {
            writer_mng.add_bin(target, self.file_data())?;
        } else if let Some(source_file) = self.source_file.as_ref() {
            writer_mng.add_raw_copy(target, source_file, self.file_target())?;
        } else if self.source_package.is_some() {
            writer_mng.add_bin(target, &self.read_data()?)?;
        }
        Ok(())
    }

    pub(crate) fn write_to<W: io::Seek + io::Write>(
        &self,
        writer_mng: &mut WriterManager<W>,
    ) -> Result<(), XlsxError> {
        self.write_to_target(self.file_target(), writer_mng)
    }
}
