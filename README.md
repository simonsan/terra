# Terra [![crates.io](https://img.shields.io/crates/v/terra.svg)](https://crates.io/crates/terra) [![docs.rs](https://docs.rs/terra/badge.svg)](https://docs.rs/terra) [![Travis](https://img.shields.io/travis/fintelia/terra.svg)]()

Terra is work in progress large scale terrain rendering library built on top of
[wgpu](https://github.com/gfx-rs/wgpu).

![Screenshot](/screenshot.png?raw=true)

# Overview

Terra supports rendering an entire planet with details ranging in scale from
hundreds of kilometers down to centimeters. In Terra, terrain is treated as a
[heightmap](https://en.wikipedia.org/wiki/Heightmap) along with a collection of
texture maps storing the surface normal, albedo, etc.

All of this information can take quite a bit of space, so it isn't included in
this repository. Instead, the necessary files are generated at runtime and
stored in a subdirectory with the current user's [cache
directory](https://docs.rs/dirs/3.0.1/dirs/fn.cache_dir.html) (which for
instance defaults to `~/.cache/terra` on Linux).

### Level of detail (LOD)

To ensure smooth frame rates and avoid noticable "LOD popping", Terra internally
uses sphere mapped version of the [Continuous Distance-Dependent Level of
Detail](https://pdfs.semanticscholar.org/6a75/892f45b72f8765379134e8d2a4ed6a04f1b0.pdf)
algorithm.

### Incremental Generation

After downloading and reprojecting some initial datasets, Terra does the rest of
its processing using wgpu's compute shader support.

# Getting Started

Running should be as simple as:

```bash
git clone git@github.com:fintelia/terra && cd terra
cargo run --release
```

The first time you run Terra, it will download and process some large
datasets. Don't worry if you have to kill the process part way through, on
subsequent runs it will resume where it left off.

Once that step is done, you should see the main Terra window. You can navigate
with the arrow keeps, and increase/decrease your altitude via the Space and Z
keys respectively. Joystick controls are also supported if one is detected. To
exit, press Escape.

### System Requirements

* Windows or Linux operating system (Terra cannot run on MacOS because the Metal API lacks support for double precision floating point)
* A fast internet connection

# Data Sources / Credits

During operation, this library downloads and merges datasets from a variety of sources. If you integrate
it into your own project, please be sure to give proper credit to all of the following as applicable.

## Elevation data

* [ETOPO1 Global Relief Model](https://www.ngdc.noaa.gov/mgg/global)

## Orthoimagery

* [Blue Marble Next Generation](https://visibleearth.nasa.gov/view.php?id=76487)

## Textures

* [The Milky Way panorama](https://www.eso.org/public/images/eso0932a/)
