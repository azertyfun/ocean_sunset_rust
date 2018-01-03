extern crate sdl2;

use sdl2::pixels::Color;
use sdl2::rect::Point;

#[macro_use]
extern crate lazy_static;

trait ColorTrait<F> where F: Fn(u8) -> u8 {
    fn apply(&mut self, f: F);
}

impl <F> ColorTrait<F> for Color where F: Fn(u8) -> u8 {
    fn apply(&mut self, f: F) {
        self.r = f(self.r);
        self.g = f(self.g);
        self.b = f(self.b);
        self.a = f(self.a);
    }
}

// We do not want to dynamically generate the palette on every use, so we generate it lazylly when the program starts
lazy_static! {
    static ref PALETTE: [[Color; 5]; 3] = {
        // Palette's base colors generated on paletton and brightened.
        // Original colors were (29, 14, 115), (0, 101, 97), (131, 0, 80).
        let base_colors = [
            Color::RGB(47, 24, 200), // BLUE
            Color::RGB(0, 200, 190), // CYAN
            Color::RGB(200, 0, 123), // RED
        ];

        // We generate a palette with, for each base color, a gradient of 5 colors from brightest to darkest
        // For a total of 15 colors
        let mut palette: [[Color; 5]; 3] = [[Color::RGB(0, 0, 0); 5]; 3];

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
                Color::RGB(0u8, 0u8, 0u8),
            ]
        }
        
        palette
    };
}

// Dimensions of the canvas
static WIDTH: i32 = 640;
static HEIGHT: i32 = 480;

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

enum BaseColor {
    Blue,
    Cyan,
    Red
}

// palette() returns an Rgb value for a given color and value in [0; 1]
fn palette(primary: BaseColor, value: f64) -> Color {
    if value > 1.0 || value < 0.0 {
        panic!("value should be in [0; 1]!");
    }

    PALETTE[match primary {
        BaseColor::Blue => 0, BaseColor::Cyan => 1, BaseColor::Red => 2
    }][((1.0 - value) * 4.0).round() as usize]
}

// dist() returns the geometric distance between two points
fn dist(x: (f64, f64), y: (f64, f64)) -> f64 {
    ((x.1 - x.0)*(x.1 - x.0) + (y.1 - y.0)*(y.1 - y.0)).sqrt()
}

// background() returns the background color for a given pixel
// This is done using a maximum brightness circle for the sun, and dimmer concentric circles for the sunset effect
fn background(x: i32, y: i32) -> Color {
    let w = WIDTH as f64;
    let h = HEIGHT as f64;

    let mut color;

    // Sun reflection
    if y > LINES_TOP {
        return if (x as f64 - WIDTH as f64 / 2.0) * (x as f64 - WIDTH as f64 / 2.0) / (SUN_REFLECTION_A * SUN_REFLECTION_A) + (y as f64 - SUN_POSITION_Y) * (y as f64 - SUN_POSITION_Y) / (SUN_REFLECTION_B * SUN_REFLECTION_B) < 1.0 {
            palette(BaseColor::Red, 0.2)
        } else {
            palette(BaseColor::Red, 0.0)
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

    palette(BaseColor::Red, color)
}

// make_lines() is responsible for creating the cyan lines. It is also responsible for handling the animation for a given v_offset.
fn make_lines(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, v_offset: u32) -> bool {
    canvas.set_draw_color(palette(BaseColor::Cyan, 1.0));

    /*
     * Vertical lines
     */

    for i in -N_VERT_LINES/2..N_VERT_LINES/2+1 {
        let start_rel = 30.0 * i as f64 / N_VERT_LINES as f64; // in [-1; 1]
        let end_rel = 2.0 * i as f64 / N_VERT_LINES as f64; // in [-1; 1]
        let mut start = (((start_rel + 1.0) * WIDTH as f64 / 2.0) as i32, HEIGHT as i32);
        let mut end = (((end_rel + 1.0) * WIDTH as f64 / 2.0) as i32, LINES_TOP);

        canvas.draw_line(Point::new(start.0, start.1), Point::new(end.0, end.1)).unwrap();
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
    canvas.draw_line(Point::new(0, LINES_TOP), Point::new(WIDTH, LINES_TOP)).unwrap();
    canvas.draw_line(Point::new(0, LINES_TOP + (v_offset as f64 * MINIMUM_SPEED) as i32), Point::new(WIDTH, LINES_TOP + (v_offset as f64 * MINIMUM_SPEED) as i32)).unwrap();
    for i in LINES_TOP as i32..HEIGHT as i32 {
        if i < LINES_TOP {
            steps_without_line += 1;
            continue;
        }

        let dist_from_top = (i as f64 - LINES_TOP as f64) / (HEIGHT as f64 - LINES_TOP as f64); // in [0; 1]
        let next_scan_line = ((LINES_MAX_DISTANCE - LINES_MIN_DISTANCE) as f64 * dist_from_top + LINES_MIN_DISTANCE as f64) as u32;

        if steps_without_line as u32 >= next_scan_line {
            canvas.draw_line(Point::new(0, i + (v_offset as f64 * (dist_from_top + MINIMUM_SPEED)) as i32), Point::new(WIDTH, i + (v_offset as f64 * (dist_from_top + MINIMUM_SPEED)) as i32)).unwrap();
            steps_without_line = 0;
        }

        steps_without_line += 1;
    }

    false
}

// build_img() is responsible for making the image file for a given offset i
fn build_img(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, i: u32) {
    // Background
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            let mut pixel = background(x, y);

            // Scan lines effect
            if y % 2 == 0 {
                pixel.r /= 2;
                pixel.g /= 2;
                pixel.b /= 2;
            }

            canvas.set_draw_color(pixel);
            canvas.draw_point(Point::new(x, y)).unwrap();
        }
    }

    // Cyan lines
    make_lines(canvas, i);
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsys = sdl_context.video().unwrap();
    let window = video_subsys.window("Ocean Sunset Rust", WIDTH as u32, HEIGHT as u32).position_centered().opengl().build().unwrap();
    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(128, 128, 128));
    canvas.clear();
    canvas.present();

    let mut events = sdl_context.event_pump().unwrap();
    let mut i = 0u64;
    'main: loop {
        for event in events.poll_iter() {
            match event {
                sdl2::event::Event::Quit {..} => break 'main,
                _ => ()
            }
        }

        build_img(&mut canvas, (i % LINES_MAX_DISTANCE as u64) as u32);
        canvas.present();
        i += 1;
    }
}
