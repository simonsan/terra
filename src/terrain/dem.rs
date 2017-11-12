use lru_cache::LruCache;
use safe_transmute;
use zip::ZipArchive;

use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{Cursor, Read};
use std::str::FromStr;
use std::{env, mem};

use cache::WebAsset;

#[derive(Debug)]
pub enum DemError {
    ParseError,
}
impl Error for DemError {
    fn description(&self) -> &str {
        "failed to parse DEM"
    }
}
impl Display for DemError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg_attr(rustfmt, rustfmt_skip)]
const EARTHDATA_WARNING: &'static str =
    "WARNING: Earthdata credentials from https://urs.earthdata.nasa.gov//users/new are \
     required to download elevation data. Once you have them, please remember to \
     `export EARTHDATA_CREDENTIALS=\"user:pass\"`";


#[derive(Copy, Clone)]
pub enum DemSource {
    Usgs30m,
    Usgs10m,
    Srtm30m,
}
impl DemSource {
    pub(crate) fn url_str(&self) -> &str {
        match *self {
            DemSource::Usgs30m => {
                "https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1/GridFloat/"
            }
            DemSource::Usgs10m => {
                "https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/13/GridFloat/"
            }
            DemSource::Srtm30m => "https://e4ftl01.cr.usgs.gov/SRTM/SRTMGL1.003/2000.02.11/",
        }
    }
    pub(crate) fn directory_str(&self) -> &str {
        match *self {
            DemSource::Usgs30m => "dems/ned1",
            DemSource::Usgs10m => "dems/ned13",
            DemSource::Srtm30m => "dems/srtm1",
        }
    }
    /// Returns the approximate resolution of data from this source in meters.
    pub(crate) fn resolution(&self) -> u32 {
        match *self {
            DemSource::Usgs30m => 30,
            DemSource::Usgs10m => 10,
            DemSource::Srtm30m => 30,
        }
    }
    /// Returns the size of cells from this data source in arcseconds.
    pub(crate) fn cell_size(&self) -> f32 {
        match *self {
            DemSource::Usgs30m => 1.0,
            DemSource::Usgs10m => 1.0 / 3.0,
            DemSource::Srtm30m => 1.0,
        }
    }
}

pub struct DigitalElevationModelParams {
    pub latitude: i16,
    pub longitude: i16,
    pub source: DemSource,
}
impl WebAsset for DigitalElevationModelParams {
    type Type = DigitalElevationModel;

    fn url(&self) -> String {
        let (latitude, longitude) = match self.source {
            DemSource::Usgs30m |
            DemSource::Usgs10m => (self.latitude + 1, self.longitude),
            _ => (self.latitude, self.longitude),
        };

        let n_or_s = if latitude >= 0 { 'n' } else { 's' };
        let e_or_w = if longitude >= 0 { 'e' } else { 'w' };

        match self.source {
            DemSource::Usgs30m |
            DemSource::Usgs10m => {
                format!(
                    "{}{}{:02}{}{:03}.zip",
                    self.source.url_str(),
                    n_or_s,
                    latitude.abs(),
                    e_or_w,
                    longitude.abs()
                )
            }
            DemSource::Srtm30m => {
                format!(
                    "{}{}{:02}{}{:03}.SRTMGL1.hgt.zip",
                    self.source.url_str(),
                    n_or_s.to_uppercase().next().unwrap(),
                    latitude.abs(),
                    e_or_w.to_uppercase().next().unwrap(),
                    longitude.abs()
                )
            }
        }
    }
    fn filename(&self) -> String {
        let n_or_s = if self.latitude >= 0 { 'n' } else { 's' };
        let e_or_w = if self.longitude >= 0 { 'e' } else { 'w' };
        format!(
            "{}/{}{:02}_{}{:03}.zip",
            self.source.directory_str(),
            n_or_s,
            self.latitude.abs(),
            e_or_w,
            self.longitude.abs()
        )
    }
    fn credentials(&self) -> Option<(String, String)> {
        match self.source {
            DemSource::Srtm30m => {
                let credentials = env::var("EARTHDATA_CREDENTIALS").expect(EARTHDATA_WARNING);
                let mut split = credentials.split(':');
                let username = split.next().unwrap_or("").to_owned();
                let password = split.next().unwrap_or("").to_owned();
                Some((username, password))
            }
            _ => None,
        }
    }

    fn parse(&self, data: Vec<u8>) -> Result<Self::Type, Box<::std::error::Error>> {
        match self.source {
            DemSource::Usgs30m |
            DemSource::Usgs10m => DigitalElevationModel::from_ned_zip(data),
            DemSource::Srtm30m => {
                DigitalElevationModel::from_srtm1_zip(self.latitude, self.longitude, data)
            }
        }
    }
}

pub struct DigitalElevationModel {
    pub width: usize,
    pub height: usize,
    pub cell_size: f64,

    pub xllcorner: f64,
    pub yllcorner: f64,

    pub elevations: Vec<f32>,
}

impl DigitalElevationModel {
    /// Load a zip file in the format for the USGS's National Elevation Dataset.
    pub fn from_ned_zip(data: Vec<u8>) -> Result<Self, Box<Error>> {
        let mut hdr = String::new();
        let mut flt = Vec::new();

        let mut zip = ZipArchive::new(Cursor::new(data))?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            if file.name().ends_with(".hdr") {
                assert_eq!(hdr.len(), 0);
                file.read_to_string(&mut hdr)?;
            } else if file.name().ends_with(".flt") {
                assert_eq!(flt.len(), 0);
                file.read_to_end(&mut flt)?;
            }
        }

        enum ByteOrder {
            LsbFirst,
            MsbFirst,
        }

        let mut width = None;
        let mut height = None;
        let mut xllcorner = None;
        let mut yllcorner = None;
        let mut cell_size = None;
        let mut byte_order = None;
        let mut nodata_value = None;
        for line in hdr.lines() {
            let mut parts = line.split_whitespace();
            let key = parts.next();
            let value = parts.next();
            if let (Some(key), Some(value)) = (key, value) {
                match key {
                    "ncols" => width = usize::from_str(value).ok(),
                    "nrows" => height = usize::from_str(value).ok(),
                    "xllcorner" => xllcorner = f64::from_str(value).ok(),
                    "yllcorner" => yllcorner = f64::from_str(value).ok(),
                    "cellsize" => cell_size = f64::from_str(value).ok(),
                    "NODATA_value" => nodata_value = f32::from_str(value).ok(),
                    "byteorder" => {
                        byte_order = match value {
                            "LSBFIRST" => Some(ByteOrder::LsbFirst),
                            "MSBFIRST" => Some(ByteOrder::MsbFirst),
                            _ => panic!("unrecognized byte order: {}", value),
                        }
                    }
                    _ => {}
                }
            }
        }

        let width = width.ok_or(DemError::ParseError)?;
        let height = height.ok_or(DemError::ParseError)?;
        let xllcorner = xllcorner.ok_or(DemError::ParseError)?;
        let yllcorner = yllcorner.ok_or(DemError::ParseError)?;
        let cell_size = cell_size.ok_or(DemError::ParseError)?;
        let byte_order = byte_order.ok_or(DemError::ParseError)?;
        let nodata_value = nodata_value.ok_or(DemError::ParseError)?;

        let size = width * height;
        if flt.len() != size * 4 {
            return Err(Box::new(DemError::ParseError));
        }

        let flt =
            unsafe { safe_transmute::guarded_transmute_many_pedantic::<u32>(&flt[..]).unwrap() };
        let mut elevations: Vec<f32> = Vec::with_capacity(size);
        for f in flt {
            let e = match byte_order {
                ByteOrder::LsbFirst => f.to_le(),
                ByteOrder::MsbFirst => f.to_be(),
            };
            let e = unsafe { mem::transmute::<u32, f32>(e) };
            elevations.push(if e == nodata_value { 0.0 } else { e });
        }

        Ok(Self {
            width,
            height,
            xllcorner,
            yllcorner,
            cell_size,
            elevations,
        })
    }

    /// Load a zip file in the format for the NASA's STRM 30m dataset.
    pub fn from_srtm1_zip(
        latitude: i16,
        longitude: i16,
        data: Vec<u8>,
    ) -> Result<Self, Box<Error>> {
        let resolution = 3601;
        let cell_size = 1.0 / 3600.0;

        let mut hgt = Vec::new();
        let mut zip = ZipArchive::new(Cursor::new(data))?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            if file.name().ends_with(".hgt") {
                assert_eq!(hgt.len(), 0);
                file.read_to_end(&mut hgt)?;
            }
        }

        assert_eq!(hgt.len(), resolution * resolution * 2);
        let hgt =
            unsafe { safe_transmute::guarded_transmute_many_pedantic::<i16>(&hgt[..]).unwrap() };
        let mut elevations: Vec<f32> = Vec::with_capacity(resolution * resolution);

        for x in 0..resolution {
            for y in 0..resolution {
                let h = i16::from_be(
                    hgt[(resolution - x - 1) + (resolution - y - 1) * resolution],
                );
                if h == -32768 {
                    elevations.push(0.0);
                } else {
                    elevations.push(h as f32);
                }
            }
        }

        Ok(Self {
            width: resolution,
            height: resolution,
            xllcorner: latitude as f64,
            yllcorner: longitude as f64,
            cell_size,
            elevations,
        })
    }

    pub fn crop(&self, width: usize, height: usize) -> Self {
        assert!(width > 0 && width <= self.width);
        assert!(height > 0 && height <= self.height);

        let xoffset = (self.width - width) / 2;
        let yoffset = (self.height - height) / 2;

        let mut elevations = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                elevations.push(self.elevations[(x + xoffset) + (y + yoffset) * self.width]);
            }
        }

        Self {
            width,
            height,
            cell_size: self.cell_size,
            xllcorner: self.xllcorner + self.cell_size * (xoffset as f64),
            yllcorner: self.yllcorner + self.cell_size * (yoffset as f64),
            elevations,
        }
    }

    pub fn get_elevation(&self, latitude: f64, longitude: f64) -> Option<f32> {
        let x = (latitude - self.xllcorner) / self.cell_size;
        let y = (longitude - self.yllcorner) / self.cell_size;

        let y = (self.height - 1) as f64 - y;

        let fx = x.floor() as usize;
        let fy = y.floor() as usize;
        if x < 0.0 || fx >= self.width - 1 || y < 0.0 || fy >= self.height - 1 {
            return None;
        }

        let h00 = self.elevations[fx + fy * self.width];
        let h10 = self.elevations[fx + 1 + fy * self.width];
        let h01 = self.elevations[fx + (fy + 1) * self.width];
        let h11 = self.elevations[fx + 1 + (fy + 1) * self.width];
        let h0 = h00 + (h01 - h00) * (y - fy as f64) as f32;
        let h1 = h10 + (h11 - h10) * (y - fy as f64) as f32;
        Some(h0 + (h1 - h0) * (x - fx as f64) as f32)
    }
}

pub struct DigitalElevationModelCache {
    source: DemSource,
    holes: HashSet<(i16, i16)>,
    dems: LruCache<(i16, i16), DigitalElevationModel>,
}

impl DigitalElevationModelCache {
    pub fn new(source: DemSource, size: usize) -> Self {
        Self {
            source,
            holes: HashSet::new(),
            dems: LruCache::new(size),
        }
    }
    pub fn get_elevation(&mut self, latitude: f64, longitude: f64) -> Option<f32> {
        let key = (latitude.floor() as i16, longitude.floor() as i16);

        if self.holes.contains(&key) {
            return None;
        }
        if let Some(dem) = self.dems.get_mut(&key) {
            return dem.get_elevation(latitude, longitude);
        }

        let dem = DigitalElevationModelParams {
            latitude: key.0,
            longitude: key.1,
            source: self.source,
        }.load();

        match dem {
            Ok(dem) => {
                let elevation = dem.get_elevation(latitude, longitude);
                assert!(elevation.is_some());
                assert!(self.dems.insert(key.clone(), dem).is_none());
                elevation
            }
            Err(_) => {
                self.holes.insert(key);
                None
            }
        }
    }
}
