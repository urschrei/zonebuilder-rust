use geo::{map_coords::MapCoordsInplace, LineString, Point, Polygon};
use geojson::GeoJson;
use std::convert::TryInto;
use std::{default::Default, iter::FromIterator};
use geographiclib_rs::{Geodesic, DirectGeodesic};

// use std::path::PathBuf;
use structopt::StructOpt;

fn true_or_false(s: &str) -> Result<bool, &'static str> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("expected `true` or `false`"),
    }
}

/// Build zones
#[derive(StructOpt, Debug)]
#[structopt(name = "zb")]
pub struct Params {
    /// Set n_circles
    #[structopt(short = "c", long, default_value = "5")]
    n_circles: usize,

    /// Number of radial segments (12 by default)
    #[structopt(short = "s", long, default_value = "12")]
    num_segments: usize,

    /// Distances between concentric rings.
    /// first 5 values of the triangular number sequence
    /// by default, entered as -d 1.0,3.0,6.0,10.0,15.0
    #[structopt(
        short,
        long,
        default_value = "1.0,3.0,6.0,10.0,15.0",
        use_delimiter = true
    )]
    distances: Vec<f64>,

    /// Number of vertices per arc
    #[structopt(short = "v", long, default_value = "5")]
    num_vertices_arc: usize,

    /// Number of decimal places in the resulting output (GeoJSON) files.
    /// Set to 6 by default. Larger numbers mean more precision but
    /// larger file sizes.
    #[structopt(short, long, default_value = "6")]
    precision: usize,

    /// Is the data projected?
    /// False by default.
    #[structopt(long, parse(try_from_str), default_value = "false")]
    projected: bool,
    // /// Output file
    // #[structopt(short, long)]
    // output: PathBuf,
}

impl Default for Params {
    fn default() -> Self {
        // default: triangular number sequence
        Params {
            n_circles: 5,
            num_segments: 12,
            distances: vec![1.0, 3.0, 6.0, 10.0, 15.0],
            num_vertices_arc: 10,
            precision: 6,
            projected: false,
        }
    }
}

fn round(poly: &mut Polygon<f64>, precision: usize) {
    // Convert precision (e.g. 5) into power of 10 (e.g. 10^5):
    let p = 10_usize.pow(precision.try_into().unwrap()) as f64;
    poly.map_coords_inplace(|&(x, y)| (f64::trunc(x * p) / p, f64::trunc(y * p) / p))
}

pub fn clockboard(
    centerpoint: Point<f64>,
    params: Params,
    // Todo: add boundary option
    //boundary: Option<Polygon<f64>>,
) -> GeoJson {
    let mut polygons = Vec::new();
    let mut irad_inner: f64;
    if params.num_segments == 1 {
        for i in params.distances {
            let zone = makecircle(
                centerpoint,
                i,
                params.num_vertices_arc * params.num_segments,
                params.projected,
            );
            polygons.push(zone);
        }
    } else {
        // For each circle radius
        for i in 0..params.distances.len() {
            let irad = params.distances[i];
            if i == 0 {
                irad_inner = 0.0;
            } else {
                irad_inner = params.distances[(i - 1)];
            }
            // For each segment
            let num_segs = if i == 0 { 1 } else { params.num_segments };
            for j in 0..num_segs {
                if i != 0 {
                    let zone = clockpoly(
                        centerpoint,
                        irad,
                        irad_inner,
                        params.num_vertices_arc,
                        params.num_segments,
                        j,
                        params.projected,
                    );
                    polygons.push(zone);
                } else {
                    let zone = makecircle(
                        centerpoint,
                        irad,
                        params.num_vertices_arc * params.num_segments,
                        params.projected,
                    );
                    polygons.push(zone);
                }
            }
        }
    }

    for polygon in &mut polygons {
        round(polygon, params.precision);
    }

    let gc = geo::GeometryCollection::from_iter(polygons);
    let fc = geojson::FeatureCollection::from(&gc);
    GeoJson::from(fc)
}

fn makecircle(centerpoint: Point<f64>, radius: f64, num_vertices: usize, projected: bool) -> Polygon<f64> {
    let mut circle_points = Vec::new();
    if projected {
        for i in 0..num_vertices {
            let angle: f64 = 2.0 * std::f64::consts::PI / (num_vertices as f64) * (i as f64);
            let x = centerpoint.x() + radius * angle.cos();
            let y = centerpoint.y() + radius * angle.sin();
            circle_points.push(Point::new(x, y));
        }
    } else {
        let mut circle_points = Vec::new();
        let crs = Geodesic::wgs84();
        for i in 0..num_vertices {
            let angle: f64 = 360.0 / (num_vertices as f64) * (i as f64);
            let (x, y, az) = crs.direct(centerpoint.x(), centerpoint.x(), angle, radius * 50000.0);
            circle_points.push(Point::new(x, y));
        }
    }
    Polygon::new(LineString::from(circle_points), vec![])
}

// Make a single clock polygon
fn clockpoly(
    centerpoint: Point<f64>,
    radius_outer: f64,
    radius_inner: f64,
    num_vertices_arc: usize,
    num_segments: usize,
    seg: usize,
    projected: bool,
) -> Polygon<f64> {
    let mut arc_outer = Vec::new();
    let mut arc_inner = Vec::new();

    // Sequence of vertices
    // in R round(seq(from, to, length.out = num_segments))
    // Number of vertices per segment
    let nv = num_vertices_arc;
    // Number of vertices in the circle
    let nc = num_vertices_arc * num_segments;
    let from_iterator = seg * nv;
    let to_iterator = 1 + (seg + 1) * nv;
    // Angle offset so first segment is North
    let o = std::f64::consts::PI / (num_segments as f64);
    if projected {
        for i in from_iterator..to_iterator {
            let angle: f64 = 2.0 * std::f64::consts::PI / (nc as f64) * (i as f64) + o;
            let x = centerpoint.x() + radius_outer * angle.sin();
            let y = centerpoint.y() + radius_outer * angle.cos();
            arc_outer.push(Point::new(x, y));
        }
        for i in (from_iterator..to_iterator).rev() {
            let angle: f64 = 2.0 * std::f64::consts::PI / (nc as f64) * (i as f64) + o;
            let x = centerpoint.x() + radius_inner * angle.sin();
            let y = centerpoint.y() + radius_inner * angle.cos();
            arc_inner.push(Point::new(x, y));
        }
    } else {
        let crs = Geodesic::wgs84();
        for i in from_iterator..to_iterator {
            let angle: f64 = 360.0 / (nc as f64) * (i as f64) + o;
            let (x, y, az) = crs.direct(centerpoint.x(), centerpoint.x(), angle, radius_outer * 50000.0);
            arc_outer.push(Point::new(x, y));
        }
        for i in (from_iterator..to_iterator).rev() {
            let angle: f64 = 360.0 / (nc as f64) * (i as f64) + o;
            let (x, y, az) = crs.direct(centerpoint.x(), centerpoint.x(), angle, radius_inner * 50000.0);
            arc_inner.push(Point::new(x, y));
        }
    }

    let arcs = [arc_outer, arc_inner].concat();
    Polygon::new(LineString::from(arcs), vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal() {
        let args: Vec<String> = Vec::new();
        let params = Params::from_iter(args);
        eprintln!("{:?}", params); // print parameters
        let gj = clockboard(Point::new(0.0, 0.0), params);
        if let GeoJson::FeatureCollection(fc) = gj {
            assert_eq!(49, fc.features.len());
        } else {
            panic!("not a feature collection");
        }
    }
}
