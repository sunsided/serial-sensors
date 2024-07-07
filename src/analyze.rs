use std::path::PathBuf;

use colorgrad::Gradient;
use glob::glob;
use itertools::izip;
use plotters::prelude::*;
use polars::prelude::*;

pub fn analyze_dump(input: PathBuf, _output: PathBuf) -> color_eyre::Result<()> {
    // Define the pattern to find all CSV files with "acc", "mag", or "gyro" in their names
    let pattern = input.join("*.csv");

    // Iterate over each file that matches the pattern
    for entry in glob(&format!("{}", pattern.display())).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                // Check if the file name contains "acc", "mag", or "gyro"
                if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                    if file_name.contains("acc")
                        || file_name.contains("mag")
                        || file_name.contains("gyro")
                    {
                        println!("Processing {file_name}");
                        let out_file_name = format!("{}.bmp", path.display());

                        // Read the CSV file using Polars
                        let df = CsvReadOptions::default()
                            .with_infer_schema_length(Some(100))
                            .with_has_header(true)
                            .try_into_reader_with_file_path(Some(path.clone()))?
                            .finish()?;

                        println!("{:?}", df.get_column_names());

                        // Normalize data time to the first observation.
                        // NOTE: This makes correlation of series between sensors a bit harder.
                        let host_time = df.column("host_time")?.cast(&DataType::Float64)?;
                        let first: f64 = host_time.get(0)?.try_extract()?;
                        let time: Vec<f32> = (host_time - first)
                            .cast(&DataType::Float32)?
                            .f32()?
                            .into_no_null_iter()
                            .collect();
                        let last: f32 = *time.last().unwrap();

                        let time_normalized: Vec<f32> = time.iter().map(|t| t / last).collect();

                        // Fetch the axis values.
                        let x: Vec<f32> = df
                            .column("x")?
                            .cast(&DataType::Float32)?
                            .f32()?
                            .into_no_null_iter()
                            .collect();
                        let y: Vec<f32> = df
                            .column("y")?
                            .cast(&DataType::Float32)?
                            .f32()?
                            .into_no_null_iter()
                            .collect();
                        let z: Vec<f32> = df
                            .column("z")?
                            .cast(&DataType::Float32)?
                            .f32()?
                            .into_no_null_iter()
                            .collect();

                        // Min and max ranges.
                        let x_min = x
                            .iter()
                            .copied()
                            .min_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap();
                        let x_max = x
                            .iter()
                            .copied()
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap();
                        let y_min = y
                            .iter()
                            .copied()
                            .min_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap();
                        let y_max = y
                            .iter()
                            .copied()
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap();
                        let z_min = z
                            .iter()
                            .copied()
                            .min_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap();
                        let z_max = z
                            .iter()
                            .copied()
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap();

                        let min = x_min.min(y_min).min(z_min);
                        let max = x_max.max(y_max).max(z_max);

                        let max = max.abs().max(min.abs()) * 1.1;
                        let min = -max;

                        let root_area =
                            BitMapBackend::new(&out_file_name, (512 * 3, 1024)).into_drawing_area();
                        root_area.fill(&WHITE)?;

                        // Custom colors
                        // let red = RGBColor(255, 127, 80); // Coral
                        // let green = RGBColor(152, 251, 152); // Mint
                        // let blue = RGBColor(135, 206, 250); // Teal
                        let red = RGBColor(220, 100, 60); // Darker Coral
                        let green = RGBColor(100, 200, 100); // Darker Mint
                        let blue = RGBColor(70, 130, 180); // Darker Teal
                        let gradient = colorgrad::oranges();

                        // TODO: split_evenly
                        let (upper, lower) = root_area.split_vertically(512);

                        let x_axis = (0.0..last).step(0.1);

                        let mut cc = ChartBuilder::on(&upper)
                            .margin(10)
                            .set_all_label_area_size(50)
                            .caption(file_name, ("sans-serif", 40))
                            .build_cartesian_2d(x_axis, min..max)?;

                        cc.configure_mesh()
                            .x_labels(20)
                            .y_labels(10)
                            .x_desc("time (seconds)")
                            .y_desc("axis readings")
                            .x_label_formatter(&|v| format!("{:.1}", v))
                            .y_label_formatter(&|v| format!("{:.1}", v))
                            .max_light_lines(4)
                            .draw()?;

                        cc.draw_series(
                            time.iter()
                                .zip(x.iter())
                                .map(|(&t, &x)| Circle::new((t, x), 1, red.filled())),
                        )?
                        .label("X")
                        .legend(|(x, y)| Circle::new((x, y), 2, red.filled()));

                        cc.draw_series(
                            time.iter()
                                .zip(y.iter())
                                .map(|(&t, &y)| Circle::new((t, y), 1, green.filled())),
                        )?
                        .label("Y")
                        .legend(|(x, y)| Circle::new((x, y), 2, green.filled()));

                        cc.draw_series(
                            time.iter()
                                .zip(z.iter())
                                .map(|(&t, &z)| Circle::new((t, z), 1, blue.filled())),
                        )?
                        .label("Z")
                        .legend(|(x, y)| Circle::new((x, y), 2, blue.filled()));

                        cc.configure_series_labels()
                            .position(SeriesLabelPosition::LowerLeft)
                            .border_style(BLACK)
                            .background_style(WHITE.mix(0.5))
                            .draw()?;

                        let (left, right) = lower.split_horizontally(512);
                        let (middle, right) = right.split_horizontally(512);

                        let plots = [
                            (left, &x, &y, "X", "Y", "X/Y"),
                            (middle, &x, &z, "X", "Z", "X/Z"),
                            (right, &y, &z, "Y", "Z", "Y/Z"),
                        ];

                        for (plot, a, b, a_desc, b_desc, label) in plots {
                            let mut cc = ChartBuilder::on(&plot)
                                .margin(5)
                                .set_all_label_area_size(50)
                                .caption(label, ("sans-serif", 10))
                                .set_label_area_size(LabelAreaPosition::Left, 40)
                                .set_label_area_size(LabelAreaPosition::Bottom, 40)
                                .build_cartesian_2d(min..max, min..max)?;

                            cc.configure_mesh()
                                .x_labels(10)
                                .y_labels(10)
                                .x_desc(a_desc)
                                .y_desc(b_desc)
                                .x_label_formatter(&|v| format!("{:.1}", v))
                                .y_label_formatter(&|v| format!("{:.1}", v))
                                .max_light_lines(4)
                                .draw()?;

                            cc.draw_series(izip!(&time_normalized, a, b).map(
                                |(&time, &x, &y)| {
                                    Circle::new(
                                        (x, y),
                                        2,
                                        colormap(time, &gradient).mix(0.5).filled(),
                                    )
                                },
                            ))?
                            .label(label)
                            .legend(|(x, y)| Circle::new((x, y), 2, BLACK.filled()));
                        }

                        root_area.present().expect("Unable to write result to file");
                        println!("Result has been saved to {}", out_file_name);
                    }
                }
            }
            Err(e) => eprintln!("Failed to read path: {:?}", e),
        }
    }
    Ok(())
}

fn colormap(value: f32, gradient: &Gradient) -> RGBAColor {
    let color = gradient.at(value as _);
    RGBAColor(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        color.a,
    )
}
