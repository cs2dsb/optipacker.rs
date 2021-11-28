#![cfg_attr(feature = "backtrace", feature(backtrace))]
use std::{
    fs::{
        self,
        File,
    },
    path::{ Path, PathBuf },
    ffi::OsStr,
    time::Instant,
    collections::BTreeMap,
};
use thiserror::Error;
use walkdir::WalkDir;
use oxipng::{
    Options as OxiOptions,
    optimize,
    InFile,
    OutFile,
};
#[cfg(feature = "zopfli")]
use oxipng::Deflaters;
use texture_packer::{
    exporter::ImageExporter,
    importer::ImageImporter,
    texture::Texture,
    MultiTexturePacker,
    TexturePackerConfig,
};
use image::ImageFormat;
use handlebars::Handlebars;
use serde::Serialize;
use regex::Regex;
use pathdiff::diff_paths;

const EXT: &str = "png";
pub const DEFAULT_TEXTURE_MAX_WIDTH: u32 = 4096;
pub const DEFAULT_TEXTURE_MAX_HEIGHT: u32 = 4096;
pub const DEFAULT_FRAME_REGEX: &str = r"(?P<name>.+)_(?P<frame>\d+)";
const DEFAULT_TEMPLATE: &str = include_str!("templates/bevy.rs.tmpl");

#[derive(Debug, Serialize, Default)]
pub struct PackedPage {
    name: String,
    path: String,
    w: u32,
    h: u32,
    items: Vec<PageItem>,
}

#[derive(Debug, Serialize)]
pub struct PageItem {
    name: String,
    frames: Vec<Frame>,
}

#[derive(Debug, Serialize)]
pub struct Frame {
    i: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error")]
    IoError(#[from] std::io::Error,
        #[cfg(feature = "backtrace")]
        #[backtrace] std::backtrace::Backtrace,
    ),

    #[error("WalkDir error")]
    WalkDirError(#[from] walkdir::Error,
        #[cfg(feature = "backtrace")]
        #[backtrace] std::backtrace::Backtrace,
    ),

    #[error("StripPrefixError")]
    StripPrefixError(#[from] std::path::StripPrefixError,
        #[cfg(feature = "backtrace")]
        #[backtrace] std::backtrace::Backtrace,
    ),

    #[error("PngError")]
    PngError(#[from] oxipng::PngError,
        #[cfg(feature = "backtrace")]
        #[backtrace] std::backtrace::Backtrace,
    ),

    #[error("TemplateError")]
    TemplateError(#[from] handlebars::TemplateError,
        #[cfg(feature = "backtrace")]
        #[backtrace] std::backtrace::Backtrace,
    ),

    #[error("RenderError")]
    RenderError(#[from] handlebars::RenderError,
        #[cfg(feature = "backtrace")]
        #[backtrace] std::backtrace::Backtrace,
    ),

    #[error("RegexCaptureError")]
    RegexCaptureError(String),

    #[error("NonUnicodeFilename")]
    NonUnicodeFilename(String),

}


#[derive(Debug, Clone, PartialEq)]
pub enum Template {
    Str(&'static str),
    String(String),
    Path(PathBuf),
    None,
}

impl Template {
    pub fn is_some(&self) -> bool {
        self != &Template::None
    }
}

/// Options to the packing and optimization process
#[derive(Debug, Clone)]
pub struct Options {
    /// The path to scan for png files
    pub input_path: PathBuf,
    /// Path to output optimized png images to
    ///
    /// It is fine for this to be under input_path, it will be
    /// excluded from the scan process
    pub optimized_path: PathBuf,
    /// Path to output packed atlas pages to
    ///
    /// It is fine for this to be under input_path, it will be
    /// excluded from the scan process
    pub packed_path: PathBuf,
    /// Handlebars template to render after the optimization process
    /// `None` disables the template
    /// `Path` points to a file that should be loaded and used
    /// `Str` and `String` directly contain the string to be used
    pub template: Template,
    /// Path to output the rendered template to
    pub template_out_path: PathBuf,
    /// Maximum width for the texture pages
    pub texture_max_width: u32,
    /// Maximum height for the texture pages
    pub texture_max_height: u32,
    /// Disable checking if the input files are newer than the output files and just
    /// redo the whole process every run
    pub skip_fresh_checks: bool,
    /// Regex used to extract the name and frame number from file names
    /// `None` disables frame matching and produces a 1 frame animation for each file
    /// `Some(Regex)` must contain named captures for `name` and `frame`. See the default regex for syntax
    ///
    pub frame_regex: Option<Regex>,
}

impl Default for Options {
    fn default() -> Self {
        Self::from_base_path("assets")
    }
}

impl Options {
    pub fn from_base_path<S: AsRef<OsStr> + ?Sized>(base_path: &S) -> Self {
        let base_path = Path::new(base_path);
        Self {
            input_path: base_path.join("textures"),
            optimized_path: base_path.join("textures/optimized"),
            packed_path: base_path.join("textures/packed"),
            template: Template::Str(DEFAULT_TEMPLATE),
            template_out_path: base_path.join("src/packed_assets.rs"),
            texture_max_width: DEFAULT_TEXTURE_MAX_WIDTH,
            texture_max_height: DEFAULT_TEXTURE_MAX_HEIGHT,
            skip_fresh_checks: false,
            frame_regex: Some(Regex::new(DEFAULT_FRAME_REGEX).unwrap()),
        }
    }
}

pub fn optimize_and_pack(options: Options) -> Result<(), Error> {
    eprintln!("========= optimize_and_pack =========");

    eprintln!("input_path: {:?}", options.input_path);
    eprintln!("optimized_path: {:?}", options.optimized_path);
    eprintln!("packed_path: {:?}", options.packed_path);
    eprintln!("template: {:?}", options.template);
    eprintln!("template_out_path: {:?}", options.template_out_path);


    let handlebars = if options.template.is_some() {
        println!("cargo:rerun-if-changed={:?}", options.template_out_path);

        let mut handlebars = Handlebars::new();
        match options.template {
            Template::Str(t) => handlebars.register_template_string("template", t)?,
            Template::String(t) => handlebars.register_template_string("template", &t)?,
            Template::Path(path) => {
                let t = fs::read_to_string(path)?;
                handlebars.register_template_string("template", &t)?
            },
            Template::None => unreachable!(),
        }

        Some(handlebars)
    } else {
        None
    };


    // Create the output directories if they don't exist
    fs::create_dir_all(&options.optimized_path)?;
    fs::create_dir_all(&options.packed_path)?;

    // Get all files in the tree
    let files = {
        let absolute = WalkDir::new(&options.input_path)
            .follow_links(true)
            .into_iter()
            .map(|r| r.map(|de|
                de.into_path()))
            .collect::<Result<Vec<PathBuf>, _>>()?;

        let ext = OsStr::new(EXT);
        absolute.into_iter()
            // Filter out anything other than the images
            .filter(|p| p.extension() == Some(&ext))
            // Filter out our opti dir
            .filter(|p| !p.starts_with(&options.optimized_path))
            // Filter out our atlas dir
            .filter(|p| !p.starts_with(&options.packed_path))
            .map(|p| p
                .strip_prefix(&options.input_path)
                .map(|r| r.to_path_buf()))
            .collect::<Result<Vec<PathBuf>, _>>()?
    };

    // Track if we actually made any changes
    let mut packing_needed = false;

    let start = Instant::now();
    let mut opti_count = 0;
    let mut opti_in_bytes = 0;
    let mut opti_out_bytes = 0;

    for relative in files.iter() {
        let in_file = options.input_path.join(relative);
        let out_file = options.optimized_path.join(relative);

        let out_dir = {
            let mut d = out_file.to_owned();
            // Pop off the filename
            d.pop();
            d
        };

        let is_new = if out_file.exists() && !options.skip_fresh_checks {
            let in_modified = in_file
                .metadata()?
                .modified()?;
            let out_modified = out_file
                .metadata()?
                .modified()?;

            in_modified >= out_modified
        } else {
            true
        };

        if is_new {
            packing_needed = true;
            opti_count += 1;

            fs::create_dir_all(&out_dir)?;
            eprintln!("  Optimizing: {:?}", relative);

            optimize(
                &InFile::Path(in_file.clone()),
                &OutFile::Path(Some(out_file.clone())),
                &OxiOptions {
                    force: true,
                    #[cfg(feature = "zopfli")]
                    deflate: Deflaters::Zopfli,
                    ..OxiOptions::max_compression()
                },
            )?;


            let in_len = in_file
                .metadata()?
                .len();
            let out_len = out_file
                .metadata()?
                .len();

            opti_in_bytes += in_len as i64;
            opti_out_bytes += out_len as i64;

            eprintln!("     bytes: {} -> {}", in_len, out_len);
        }
    }

    if opti_count > 0 {
        eprintln!("Optimized {} in {:.1?}. Original bytes: {}, optimized bytes: {}, saving: {} ({:.1?}%)",
            opti_count,
            start.elapsed(),
            opti_in_bytes,
            opti_out_bytes,
            opti_in_bytes - opti_out_bytes,
            (opti_in_bytes - opti_out_bytes) as f64 / opti_in_bytes as f64 * 100.,
        );
    }

    if packing_needed {
        let mut packer = MultiTexturePacker::new_skyline(TexturePackerConfig {
            max_width: options.texture_max_width,
            max_height: options.texture_max_height,
            allow_rotation: false,
            texture_outlines: true,
            border_padding: 2,
            ..Default::default()
        });

        for relative in files.iter() {
            let out_file = options.optimized_path.join(relative);

            let name = {
                //TODO: better error handling
                out_file.file_stem().unwrap().to_owned()
            };

            //TODO: better error handling
            let texture = ImageImporter::import_from_file(&out_file).unwrap();

            packer.pack_own(name, texture).unwrap();
        };


        let start = Instant::now();
        let mut opti_count = 0;
        let mut opti_in_bytes = 0;
        let mut opti_out_bytes = 0;

        let mut pages = Vec::new();

        for (i, page) in packer.get_pages().iter().enumerate() {

            println!("#{} | Dimensions : {}x{}", i, page.width(), page.height());
            for (name, frame) in page.get_frames() {
                println!("#{} |   {:7?} : {:?}", i, name, frame.frame);
            }

            let page_name = format!("page_{}", i);
            let template_out_dir = options.template_out_path.parent().unwrap_or(Path::new("."));
            let page_path = diff_paths(&options.packed_path, template_out_dir).unwrap_or(PathBuf::from("."));
            let page_file_name = format!("{}.png", &page_name);
            let page_file_path = page_path
                .join(&page_file_name)
                .iter()
                // The first part of the path will be the base dir passed to the game engine
                .skip(1)
                .collect::<PathBuf>()
                .to_str().ok_or(Error::NonUnicodeFilename(
                    format!("Path '{:?}' is contains non-unicode characters which is not supported here", page_path)))?
                .to_owned();

            let exporter = ImageExporter::export(page).unwrap();
            let out_file = options.packed_path.join(&page_file_name);
            let mut file = File::create(&out_file)?;
            exporter.write_to(&mut file, ImageFormat::Png).unwrap();

            opti_count += 1;

            let in_len = out_file
                .metadata()?
                .len();

            eprintln!("  Optimizing: {:?}", out_file);

            optimize(
                &InFile::Path(out_file.clone()),
                &OutFile::Path(None),
                &OxiOptions {
                    force: true,
                    #[cfg(feature = "zopfli")]
                    deflate: Deflaters::Zopfli,
                    ..OxiOptions::max_compression()
                },
            )?;

            let out_len = out_file
                .metadata()?
                .len();

            opti_in_bytes += in_len as i64;
            opti_out_bytes += out_len as i64;


            let mut items = BTreeMap::new();
            for (name, frame) in page.get_frames() {
                let name = name.to_str().ok_or(Error::NonUnicodeFilename(
                    format!("File '{:?}' is contains non-unicode characters which is not supported here", name)))?
                    .to_owned();

                let (name, frame_number) = if let Some(regex) = &options.frame_regex {
                    if let Some(caps) = regex.captures(&name) {
                        if caps.len() < 3 {
                            Err(Error::RegexCaptureError(
                                format!("Frame regex capture matched (file: {}) but only had {} capture groups (expect 3: full match, name, frame)",
                                    name, caps.len())))?;
                        }

                        let name = caps.name("name").unwrap().as_str().to_owned();
                        let frame_str = caps.name("frame").unwrap().as_str().to_owned();
                        let frame = frame_str.parse::<u32>().map_err(|e| Error::RegexCaptureError(
                            format!("Failed to parse frame number captured by regex: {:?}", e)))?;

                        (name, frame)
                    } else {
                        eprintln!("Failed to match {}", &name);
                        (name, 0)
                    }
                } else {
                    (name, 0)
                };

                if !items.contains_key(&name) {
                    items.insert(name.clone(), PageItem {
                        name: name.clone(),
                        frames: Default::default(),
                    });
                }

                let item = items.get_mut(&name).unwrap();

                let rect = frame.frame;
                item.frames.push(Frame {
                    i: frame_number,
                    x: rect.x,
                    y: rect.y,
                    w: rect.w,
                    h: rect.h,
                });
            }

            let items = {
                let mut items: Vec<_> = items
                    .into_values()
                    .collect();

                for PageItem { frames, .. } in items.iter_mut() {
                    frames.sort_by(|a, b| a.i.cmp(&b.i));
                }

                items
            };

            pages.push(PackedPage {
                name: page_name,
                //TODO: add relative
                path: page_file_path,
                w: page.width(),
                h: page.height(),
                items,
            });
        }

        eprintln!("Optimized {} atlases in {:.1?}. Original bytes: {}, optimized bytes: {}, saving: {} ({:.1?}%)",
            opti_count,
            start.elapsed(),
            opti_in_bytes,
            opti_out_bytes,
            opti_in_bytes - opti_out_bytes,
            (opti_in_bytes - opti_out_bytes) as f64 / opti_in_bytes as f64 * 100.,
        );

        if let Some(handlebars) = handlebars {
            let mut data = BTreeMap::new();
            data.insert("pages".to_owned(), pages);

            let out = handlebars.render("template", &data)?;
            eprintln!("handlebars output: {}", out);

            fs::write(options.template_out_path, out)?;
        }
    }

    eprintln!("========= optimize_and_pack =========");

    Ok(())
}
