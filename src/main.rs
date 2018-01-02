extern crate image;

use std::fs::File;
use image::{ImageBuffer, Rgb, Pixel};

#[macro_use]
extern crate lazy_static;

// We do not want to dynamically generate the palette on every use, so we generate it lazylly when the program starts
lazy_static! {
    static ref PALETTE: [[Rgb<u8>; 5]; 3] = {
        // Palette's base colors generated on paletton and brightened.
        // Original colors were (29, 14, 115), (0, 101, 97), (131, 0, 80).
        let base_colors = [
            Rgb([47, 24, 200]), // BLUE
            Rgb([0, 200, 190]), // CYAN
            Rgb([200, 0, 123]), // RED
        ];

        // We generate a palette with, for each base color, a gradient of 5 colors from brightest to darkest
        // For a total of 15 colors
        let mut palette: [[Rgb<u8>; 5]; 3] = [[Rgb([0, 0, 0]); 5]; 3];

        for i in 0..3 {
            palette[i] = [
                base_colors[i],
                {
                    let mut color = base_colors[i].clone();
                    color.apply(|v| {
                        (v as f64 * 0.75) as u8
                    });
                    color
                },
                {
                    let mut color = base_colors[i].clone();
                    color.apply(|v| {
                        (v as f64 * 0.5) as u8
                    });
                    color
                },
                {
                    let mut color = base_colors[i].clone();
                    color.apply(|v| {
                        (v as f64 * 0.25) as u8
                    });
                    color
                },
                Rgb([0u8, 0u8, 0u8]),
            ]
        }
        
        palette
    };
}

// Dimensions of the canvas
static WIDTH: u32 = 640;
static HEIGHT: u32 = 480;

// Vertical position of the sun
static SUN_POSITION_Y: f64 = 220.0;

// Number of cyan vertical lines
static N_VERT_LINES: i32 = 80;

// Starting vertical position of the lines (from the top)
static LINES_TOP: i32 = 240;

// Maximum distance between two horizontal lines (at the bottom)
static LINES_MAX_DISTANCE: u32 = 50;
// Minimum distance between two horizontal lines (at the top)
static LINES_MIN_DISTANCE: u32 = 10;
// Minimum speed modifier of the animation (so that the top line doesn't stay in place)
static MINIMUM_SPEED: f64 = 0.2;

// a and b values of the reflection ellipse
static SUN_REFLECTION_A: f64 = 100.0;
static SUN_REFLECTION_B: f64 = 350.0;

enum Color {
    Blue,
    Cyan,
    Red
}

// palette() returns an Rgb value for a given color and value in [0; 1]
fn palette(primary: Color, value: f64) -> Rgb<u8> {
    if value > 1.0 || value < 0.0 {
        panic!("value should be in [0; 1]!");
    }

    PALETTE[match primary {
        Color::Blue => 0, Color::Cyan => 1, Color::Red => 2
    }][((1.0 - value) * 4.0).round() as usize]
}

// dist() returns the geometric distance between two points
fn dist(x: (f64, f64), y: (f64, f64)) -> f64 {
    ((x.1 - x.0)*(x.1 - x.0) + (y.1 - y.0)*(y.1 - y.0)).sqrt()
}

// background() returns the background color for a given pixel
// This is done using a maximum brightness circle for the sun, and dimmer concentric circles for the sunset effect
fn background(x: u32, y: u32) -> Rgb<u8> {
    let w = WIDTH as f64;
    let h = HEIGHT as f64;

    let mut color;

    // Sun reflection
    if y > LINES_TOP as u32 {
        return if (x as f64 - WIDTH as f64 / 2.0) * (x as f64 - WIDTH as f64 / 2.0) / (SUN_REFLECTION_A * SUN_REFLECTION_A) + (y as f64 - SUN_POSITION_Y) * (y as f64 - SUN_POSITION_Y) / (SUN_REFLECTION_B * SUN_REFLECTION_B) < 1.0 {
            palette(Color::Red, 0.2)
        } else {
            palette(Color::Red, 0.0)
        };
    }

    let distance = dist((x as f64, w/2.0), (y as f64, SUN_POSITION_Y)); // Distance from the center of the sun
    let max_distance = (w*w + h*h).sqrt() / 1.5; // Greater than the maximum distance from the center of the sun we will ever see, which is the diagonal of the screen √(width² + height²). This is not an ideal value (it could be further reduced), but this looks good enough for the gradient effect.

    if distance > 75.0 {
        color = 0.65 - distance / max_distance;
        
        if color < 0.0 {
            color = 0.0;
        }
    } else {
        color = 1.0;
    }

    palette(Color::Red, color)
}

// make_line() draws a line using the Digital Differential Analyzer algorithm (DDA). It stores the pixels belonging to the line in the `lines` vector.
fn make_line(lines: &mut Vec<(u32, u32)>, start: &mut (i32, i32), end: &mut (i32, i32)) {
    let m = (end.1 as f64 - start.1 as f64) / (end.0 as f64 - start.0 as f64);

    if m < 0.0 && start.0 < end.0 {
        let tmp = *start;
        *start = *end;
        *end = tmp;
    }

    let dx = end.0 - start.0;
    let dy = end.1 - start.1;

    let steps = if dx.abs() > dy.abs() {
        dx.abs()
    } else {
        dy.abs()
    };

    let x_inc = dx as f64 / steps as f64;
    let y_inc = dy as f64 / steps as f64;

    let mut x_k = start.0 as f64;
    let mut y_k = start.1 as f64;

    for _ in 0..steps {
        x_k += x_inc;
        y_k += y_inc;

        lines.push((x_k.round() as u32, y_k.round() as u32));
    }
}

// make_lines() is responsible for creating the cyan lines. It is also responsible for handling the animation for a given v_offset.
fn make_lines(lines: &mut Vec<(u32, u32)>, v_offset: u32) -> bool {
    /*
     * Vertical lines
     */

    for i in -N_VERT_LINES/2..N_VERT_LINES/2+1 {
        let start_rel = 30.0 * i as f64 / N_VERT_LINES as f64; // in [-1; 1]
        let end_rel = 2.0 * i as f64 / N_VERT_LINES as f64; // in [-1; 1]
        let mut start = (((start_rel + 1.0) * WIDTH as f64 / 2.0) as i32, HEIGHT as i32);
        let mut end = (((end_rel + 1.0) * WIDTH as f64 / 2.0) as i32, LINES_TOP);

        make_line(lines, &mut start, &mut end);
    }


    /*
     * Horizontal lines
     */

    let mut steps_without_line = 0;

    /* Invariant:
     * steps_without_line is the number of times we looped without drawing a line;
     * dist_from_top is the distance relative from the top, from 0 to 1, for the current scan line;
     * next_scan_line is the number of steps that must be done before drawing the next line.
     */
    make_line(lines, &mut (0i32, LINES_TOP as i32), &mut (WIDTH as i32, LINES_TOP as i32)); // There must always be a top-most line meeting the end of the vertical lines to not look weird
    make_line(lines, &mut (0i32, LINES_TOP + (v_offset as f64 * MINIMUM_SPEED) as i32), &mut (WIDTH as i32, LINES_TOP + (v_offset as f64 * MINIMUM_SPEED) as i32)); // We write the top-most line manually to compensate for the disappearing bottom-most line (comment this line to see what happens otherwise)
    for i in LINES_TOP as i32..HEIGHT as i32 {
        if i < LINES_TOP {
            steps_without_line += 1;
            continue;
        }

        let dist_from_top = (i as f64 - LINES_TOP as f64) / (HEIGHT as f64 - LINES_TOP as f64); // in [0; 1]
        let next_scan_line = ((LINES_MAX_DISTANCE - LINES_MIN_DISTANCE) as f64 * dist_from_top + LINES_MIN_DISTANCE as f64) as u32;

        if steps_without_line as u32 >= next_scan_line {
            make_line(lines, &mut (0i32, i + (v_offset as f64 * (dist_from_top + MINIMUM_SPEED)) as i32), &mut (WIDTH as i32, i + (v_offset as f64 * (dist_from_top + MINIMUM_SPEED)) as i32));
            steps_without_line = 0;
        }

        steps_without_line += 1;
    }

    false
}

// build_img() is responsible for making the image file for a given offset i
fn build_img(i: u32) {
    let mut img = ImageBuffer::new(WIDTH, HEIGHT);

    // Background
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = background(x, y);
    }
    println!("Background done.");

    // Cyan lines
    let mut lines: Vec<(u32, u32)> = Vec::new();
    make_lines(&mut lines, i);
    for pixel in lines {
        if pixel.0 < WIDTH && pixel.1 < HEIGHT {
            *img.get_pixel_mut(pixel.0, pixel.1) = palette(Color::Cyan, 1.0);
        }
    }
    println!("Lines done.");

    // Scanline effect
    let cyan_pixel = palette(Color::Cyan, 1.0); // We do not want to apply the scanline effect to the grid pattern, as it is perfectly parallel and would therefore look weird
    for (_, y, pixel) in img.enumerate_pixels_mut() {
        if y % 2 == 0 && *pixel != cyan_pixel {
            pixel.apply(|v| {
                v / 2
            });
        }
    }
    println!("Scanlines done.");

    // Output to file
    let ref mut fout = File::create(format!("out/out{:05}.png", i)).unwrap();
    image::ImageRgb8(img).save(fout, image::PNG).unwrap();
    println!("File saved.");
}

fn main() {
    let start = std::time::Instant::now();

    // We create as many threads as there are images. Even for a large value of LINES_MAX_DISTANCE, this isn't a problem as the threads are very memory inexpensive.
    let mut handles = Vec::new();
    for i in 0..LINES_MAX_DISTANCE {
        handles.push(std::thread::spawn(move || {
            build_img(i);
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();

    println!("Generated animation in {}.{:03} s.", elapsed.as_secs(), (elapsed.subsec_nanos() as f64 / 1e6) as u32);
}
