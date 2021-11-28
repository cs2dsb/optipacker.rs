# Optipacker.rs

Opinonated image optimizer and texture atlas packer designed to be used from build.rs

To facilitate expedient builds when used from build.rs, it checks the modified dates of input and
output images and will only run if there are new or modified images in the input.

It packs any number of animations into optimal texture assets. Each page of assets is limited
to a given size that is optimal graphics card use - the default is 4096x4096 but this can be
changed using the `Options` struct.

Having packed the textures, it generates some kind of output from a
[handlebarsrs](https://github.com/sunng87/handlebars-rust) template. The default template provided
generates rust code compattible with [bevy 0.5](https://github.com/bevyengine/bevy/) and
[bevy_asset_loader](https://github.com/NiklasEi/bevy_asset_loader) but outputting ron, toml, json, etc.
should be simple enough.

## Examples

### optimize_and_pack

Shows the code that you would put in your build.rs to use this crate

### bevy_loading

Consumes the packed resources from `optimize_and_pack` (make sure you run it first) and
displays the animated sprites on the screen.

## Templates

### bevy.rs.tmpl

Generates a rust file compattible with the [bevy_asset_loader](https://github.com/NiklasEi/bevy_asset_loader)
plugin. It generates a struct called `AtlasSprites` which has a `build` function that you can use to hook it
up with bevy and the asset loader.

The `build` function uses the asset loader's `with_collection` function to register the packed texture pages.
It also registers a system that runs `on_exit` from the initial state of the loader that creates `TextureAtlas`
assets for each animation - this hooks up the texture pages and frame numbers for you so you only need to
grab the `TextureAtlas` handle from the `AtlasSprites` resource to use the animation.

## Options

The `Options` struct passed into `optimize_and_pack` controls the process. The `Default` should work for
standard bevy projects that have an `assets` folder in the root of the project - it assumes it should pack and
optimize all `png` images in the `assets/textures` folder and you want the template output to go to
`src/packed_assets.rs`. It assumes images are named `my_animation_01.png` where the frame number is always
at the end and preceeded by an underscore, everything before that underscore becomes the name.

See the rust documentation for the `Options` struct for full details on what can be configured

## Shortcomings

1. File names must be unique across all packed images - `a/character.png` and `b/character.png` will fail - likely
   when trying to use the rendered template the compiler/parser would detect duplicate keys as an error.
2. Only supports `png` images
