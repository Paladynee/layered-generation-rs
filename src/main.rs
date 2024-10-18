use image::Rgb;
use image::{self, ImageBuffer};
use rayon::prelude::*;
use std::env;
use std::io;
use std::time::{Duration, Instant};

fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!("Usage: {} <depth> <seed>", args[0]);
        return Ok(());
    }

    let depth = match args[1].parse::<usize>() {
        Ok(depth) => depth,
        Err(_) => {
            eprintln!("Could not parse the depth as an integer.");
            return Ok(());
        }
    };

    let seed = match args.get(2) {
        Some(seed) => match seed.parse::<u32>() {
            Ok(seed) => seed,
            Err(_) => {
                eprintln!("Could not parse the seed as an integer.");
                return Ok(());
            }
        },
        None => 0xcafebabe,
    };

    type Layer = ImageBuffer<Rgb<u8>, Vec<u8>>;

    let mut master_rand = Xorshift32::new(seed);
    eprintln!("Generating {} image layers...", depth);
    let (gentime, image_buffers) = time(|| {
        (0..depth)
        .map(|a| (a, master_rand.next_u32().wrapping_add(3)))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|(current_depth, seed)| {
            let width = 4 * 2u32.pow(current_depth as u32);
            let height = 3 * 2u32.pow(current_depth as u32);

            let mut imgbuf: Layer = ImageBuffer::new(width, height);
            let mut sub_rand = Xorshift32::new(seed);
            let mut intensity_buf = Vec::with_capacity(width as usize * height as usize);

            for _ in 0..width * height {
                intensity_buf.push((sub_rand.next_u32() % 256) as u8);
            }

            imgbuf
                .par_pixels_mut()
                .zip(intensity_buf)
                .for_each(|(pixel, intensity)| {
                    let pix = Rgb([intensity, intensity, intensity]);
                    *pixel = pix;
                });

            eprintln!(
                "Concurrently generated image layer {} with size {}x{} with seed {}. Total pixels in layer: {}",
                current_depth,
                width,
                height,
                seed,
                imgbuf.pixels().len()
            );

            imgbuf
        })
        .collect::<Vec<_>>()
    });

    let (final_w, final_h) = image_buffers.iter().last().unwrap().dimensions();
    let mut final_buf: Layer = ImageBuffer::new(final_w, final_h);

    let (calctime, _) = time(|| {
        final_buf
            .par_enumerate_pixels_mut()
            .for_each(|(x, y, pixel)| {
                let avgintensity = image_buffers
                    .iter()
                    .enumerate()
                    .map(|(i, imgbuf)| {
                        let divisor = 2u32.pow((image_buffers.len() - i) as u32);
                        let scaled_x = x / divisor;
                        let scaled_y = y / divisor;

                        let pixel = imgbuf.get_pixel(scaled_x, scaled_y);
                        pixel.0[0] as u32
                    })
                    .sum::<u32>()
                    / image_buffers.len() as u32;

                let pix = Rgb([avgintensity as u8, avgintensity as u8, avgintensity as u8]);
                *pixel = pix;

                let nth = y * final_w + x;
                if nth % 10_000_000 == 0 && nth != 0 {
                    eprintln!("Processed {:9>}th pixel {:6>}, {:6>}...", nth, x, y);
                }
            });
    });

    eprintln!("Total pixels calculated: {}", final_buf.pixels().len());

    eprintln!("Saving final image...");
    let (savetime, _) = time(|| final_buf.save("output.png").unwrap());

    eprintln!(
        "Timing data:\n\tGeneration: {:?}\n\tCalculation: {:?}\n\tSaving: {:?}",
        gentime, calctime, savetime
    );

    Ok(())
}

struct Xorshift32 {
    x: u32,
}

impl Xorshift32 {
    const fn new(seed: u32) -> Self {
        Self { x: seed }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.x;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.x = x;
        x
    }
}

fn time<A, T: FnMut() -> A>(mut f: T) -> (Duration, A) {
    let start = Instant::now();
    let a = f();
    (start.elapsed(), a)
}
