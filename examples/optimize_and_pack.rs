use std::path::PathBuf;
use optipacker::{
    optimize_and_pack,
    Options,
};

const ASSETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/assets");
const TEMPLATE_OUT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/packed.rs");

fn main() -> Result<(), optipacker::Error> {
    optimize_and_pack(Options {
        skip_fresh_checks: true,
        template_out_path: PathBuf::from(TEMPLATE_OUT_PATH),
        /* This is how you would load a template from the filesystem. Could also use `include_str!`
        template: optipacker::Template::Path(
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/templates/bevy.rs.tmpl"))),
        */
        ..Options::from_base_path(ASSETS_DIR)
    })
}
