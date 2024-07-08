use std::fs::File;
use std::path::PathBuf;

use colorgrad::Gradient;
use glob::glob;
use itertools::izip;
use ndarray_stats::CorrelationExt;
use plotters::coord::Shift;
use plotters::prelude::*;
use plotters::style::text_anchor::{HPos, Pos, VPos};
use polars::prelude::*;

pub fn analyze_dump(
    input: PathBuf,
    output: PathBuf,
    from: f64,
    to: Option<f64>,
) -> color_eyre::Result<()> {
    // Define the pattern to find all CSV files with "acc", "mag", or "gyro" in their names
    let pattern = input.join("*.csv");

    let mut combined = None;

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
                        let output_file = output.join(format!("{file_name}.bmp"));
                        let out_file_name = format!("{}", output_file.display());

                        let (sensor_type, _sensor_type_short) = if file_name.contains("acc") {
                            ("accelerometer", "acc")
                        } else if file_name.contains("mag") {
                            ("magnetometer", "mag")
                        } else if file_name.contains("gyro") {
                            ("gyroscope", "gyro")
                        } else {
                            ("unknown", "unknown")
                        };

                        // Get the identification file.
                        let (sensor_tag, ident) = get_ident(input.clone(), &file_name)?;
                        let label = if !sensor_tag.is_empty() && !ident.is_empty() {
                            println!("{sensor_tag} is a {ident}");
                            format!("{sensor_type} ({ident})")
                        } else {
                            println!("Unable to identify sensor");
                            format!("{sensor_type} ({file_name})")
                        };

                        // Read the CSV file using Polars
                        let df = CsvReadOptions::default()
                            .with_infer_schema_length(Some(100))
                            .with_has_header(true)
                            .try_into_reader_with_file_path(Some(path.clone()))?
                            .finish()?;

                        // Normalize data time to the first observation.
                        // NOTE: This makes correlation of series between sensors a bit harder.
                        let host_time = df.column("host_time")?.cast(&DataType::Float64)?;
                        let first: f64 = host_time.get(0)?.try_extract()?;
                        let time = host_time.clone() - first;
                        let last: f64 = time.get(time.len() - 1)?.try_extract()?;

                        // Filter to selected time range.
                        let filter_from = time.cast(&DataType::Float64)?.gt_eq(from)?;
                        let filter_to = time.cast(&DataType::Float64)?.lt_eq(to.unwrap_or(last))?;
                        let filter = filter_from & filter_to;

                        // Filter to the proper time range.
                        let host_time = host_time.filter(&filter)?;
                        let time_series = time.filter(&filter)?;

                        let time: Vec<f32> = time_series
                            .cast(&DataType::Float32)?
                            .f32()?
                            .into_no_null_iter()
                            .collect();
                        let first: f32 = *time.first().unwrap();
                        let last: f32 = *time.last().unwrap();

                        let time_normalized: Vec<f32> =
                            time.iter().map(|t| (t - first) / (last - first)).collect();

                        // Fetch data series.
                        let x_series = df.column("x")?.filter(&filter)?.cast(&DataType::Float32)?;
                        let y_series = df.column("y")?.filter(&filter)?.cast(&DataType::Float32)?;
                        let z_series = df.column("z")?.filter(&filter)?.cast(&DataType::Float32)?;

                        // Join the data frames.
                        join_datasets(
                            &mut combined,
                            &label,
                            host_time,
                            &x_series,
                            &y_series,
                            &z_series,
                        )?;

                        // Fetch the axis values.
                        let x: Vec<f32> = x_series.f32()?.into_no_null_iter().collect();
                        let y: Vec<f32> = y_series.f32()?.into_no_null_iter().collect();
                        let z: Vec<f32> = z_series.f32()?.into_no_null_iter().collect();

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

                        const BLOCK_HEIGHT: u32 = 512;
                        const BLOCK_WIDTH: u32 = 512;

                        const NUM_ROWS: u32 = 5;
                        const NUM_COLS: u32 = 4;

                        let root_area = BitMapBackend::new(
                            &out_file_name,
                            (BLOCK_WIDTH * NUM_COLS, BLOCK_HEIGHT * NUM_ROWS + 40),
                        )
                        .into_drawing_area();
                        root_area.fill(&WHITE)?;

                        // Custom colors
                        // let red = RGBColor(255, 127, 80); // Coral
                        // let green = RGBColor(152, 251, 152); // Mint
                        // let blue = RGBColor(135, 206, 250); // Teal
                        let red = RGBColor(220, 100, 60); // Darker Coral
                        let green = RGBColor(100, 200, 100); // Darker Mint
                        let blue = RGBColor(70, 130, 180); // Darker Teal
                        let gradient = colorgrad::oranges();

                        // Apply title.
                        let (upper, lower) = root_area.split_vertically(40);
                        upper.titled(&label, ("sans-serif", 40))?;

                        // Plot area.
                        let (upper, lower) = lower.split_vertically(BLOCK_HEIGHT);

                        // Plot 3D
                        let (left, right) = upper.split_horizontally(BLOCK_WIDTH);
                        let mut cc = ChartBuilder::on(&left)
                            .margin(10)
                            .build_cartesian_3d(min..max, min..max, min..max)
                            .unwrap();

                        cc.configure_axes()
                            .x_labels(20)
                            .y_labels(20)
                            .z_labels(20)
                            .max_light_lines(4)
                            .draw()?;

                        cc.draw_series(izip!(&time_normalized, &x, &y, &z).map(
                            |(&time, &x, &y, &z)| {
                                Circle::new(
                                    (x, y, z),
                                    2,
                                    colormap(time, &gradient).mix(0.5).filled(),
                                )
                            },
                        ))?
                        .label(label.clone())
                        .legend(|(x, y)| Circle::new((x, y), 2, BLACK.filled()));

                        // Plot the X/Y, X/Z, Y/Z views
                        let (left, right) = right.split_horizontally(BLOCK_WIDTH);
                        let (middle, right) = right.split_horizontally(BLOCK_WIDTH);

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

                        // Plot the combined view.
                        let (upper, lower) = lower.split_vertically(BLOCK_HEIGHT);
                        plot_combined(
                            &time, first, last, &x, &y, &z, max, min, red, green, blue, &upper,
                        )?;

                        // Plot the X view.
                        let (upper, lower) = lower.split_vertically(BLOCK_HEIGHT);

                        let time_axis = (first..last).step(0.1);
                        let mut cc = ChartBuilder::on(&upper)
                            .margin(10)
                            .set_all_label_area_size(50)
                            .build_cartesian_2d(time_axis, min..max)?;

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

                        cc.configure_series_labels()
                            .position(SeriesLabelPosition::LowerLeft)
                            .border_style(BLACK)
                            .background_style(WHITE.mix(0.5))
                            .draw()?;

                        // Plot the Y view.
                        let (upper, lower) = lower.split_vertically(BLOCK_HEIGHT);

                        let time_axis = (first..last).step(0.1);
                        let mut cc = ChartBuilder::on(&upper)
                            .margin(10)
                            .set_all_label_area_size(50)
                            .build_cartesian_2d(time_axis, min..max)?;

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
                                .zip(y.iter())
                                .map(|(&t, &y)| Circle::new((t, y), 1, green.filled())),
                        )?
                        .label("Y")
                        .legend(|(x, y)| Circle::new((x, y), 2, green.filled()));

                        cc.configure_series_labels()
                            .position(SeriesLabelPosition::LowerLeft)
                            .border_style(BLACK)
                            .background_style(WHITE.mix(0.5))
                            .draw()?;

                        // Plot the Z view.
                        let (upper, _lower) = lower.split_vertically(BLOCK_HEIGHT);

                        let time_axis = (first..last).step(0.1);
                        let mut cc = ChartBuilder::on(&upper)
                            .margin(10)
                            .set_all_label_area_size(50)
                            .build_cartesian_2d(time_axis, min..max)?;

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

                        root_area.present().expect("Unable to write result to file");
                        println!("Result has been saved to {}", out_file_name);
                    }
                }
            }
            Err(e) => eprintln!("Failed to read path: {:?}", e),
        }
    }

    if let Some(combined) = &mut combined {
        save_combined_to_csv(&output, combined)?;
        plot_cross_correlation(&output, combined)?;
    }

    Ok(())
}

fn plot_cross_correlation(
    output: &std::path::Path,
    combined: &mut DataFrame,
) -> color_eyre::Result<()> {
    println!("Calculating cross-correlation ...");
    let combined = combined.drop("host_time")?;
    let array = combined
        .to_ndarray::<Float32Type>(IndexOrder::C)?
        .reversed_axes();
    let xcorr_matrix = array.pearson_correlation().unwrap();
    println!("{xcorr_matrix}");
    println!("{} x {}", xcorr_matrix.nrows(), xcorr_matrix.ncols());

    let output_file = format!("{}", output.join("cross-correlation.bmp").display());
    println!("Plotting cross-correlation to {output_file}");

    let count = xcorr_matrix.nrows();
    let columns = combined.get_column_names();

    let root = BitMapBackend::new(&output_file, (1024, 1024)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Sensor Cross-Correlation", ("sans-serif", 40))
        .margin(10)
        .top_x_label_area_size(256)
        .y_label_area_size(256)
        .build_cartesian_2d(0.0..(count as f32), 0.0..(count as f32))?;

    let label = |idx: f32| {
        if idx < 0.0 || idx >= (count as f32) {
            return "";
        }

        columns[idx as usize]
    };

    chart
        .configure_mesh()
        .x_labels(count + 1)
        .y_labels(count + 1)
        .x_label_formatter(&|&x| label(x).to_string())
        .y_label_formatter(&|&y| label(count as f32 - y - 1.0).to_string())
        .x_label_style(
            ("sans-serif", 20)
                .into_font()
                .transform(FontTransform::Rotate270)
                .color(&BLACK)
                .pos(Pos::new(HPos::Right, VPos::Bottom)),
        )
        .y_label_style(("sans-serif", 20).into_font())
        .max_light_lines(0)
        .set_all_tick_mark_size(10.0)
        .x_label_offset(40)
        .y_label_offset(-40)
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()?;

    let gradient = &colorgrad::viridis();

    let matrix = &xcorr_matrix;
    chart.draw_series((0..count).flat_map(|row| {
        (0..count).map(move |col| {
            let value = matrix[(row, col)];
            let color = colormap((value + 1.0) * 0.5, gradient);
            Rectangle::new(
                [
                    (col as f32, count as f32 - row as f32 - 1.0),
                    (col as f32 + 1.0, count as f32 - row as f32),
                ],
                ShapeStyle {
                    color: color.to_rgba(),
                    filled: true,
                    stroke_width: 1,
                },
            )
        })
    }))?;

    chart.draw_series((0..count).flat_map(|row| {
        (0..count).map(move |col| {
            let value = matrix[(row, col)];
            let text = Text::new(
                format!("{:.2}", value),
                (col as f32 + 0.5, count as f32 - row as f32 - 0.5),
                ("sans-serif", 24.0)
                    .into_font()
                    .color(&BLACK)
                    .pos(Pos::new(HPos::Center, VPos::Center)),
            );
            text
        })
    }))?;

    root.present()?;
    Ok(())
}

fn save_combined_to_csv(
    output: &std::path::Path,
    combined: &mut DataFrame,
) -> color_eyre::Result<()> {
    let output_file = output.join("joined.csv");
    println!("Saving joined data frame to {}", output_file.display());
    let file = File::create(output_file)?;
    CsvWriter::new(file).include_header(true).finish(combined)?;
    Ok(())
}

fn join_datasets(
    combined: &mut Option<DataFrame>,
    label: &String,
    host_time: Series,
    x_series: &Series,
    y_series: &Series,
    z_series: &Series,
) -> color_eyre::Result<()> {
    match combined {
        None => {
            let mut x_series = x_series.clone();
            let mut y_series = y_series.clone();
            let mut z_series = z_series.clone();
            x_series.rename(&format!("X {label}"));
            y_series.rename(&format!("Y {label}"));
            z_series.rename(&format!("Z {label}"));
            let df = DataFrame::new(vec![host_time, x_series, y_series, z_series])?;
            let df = df.sort(
                ["host_time"],
                SortMultipleOptions::default().with_maintain_order(true),
            )?;
            *combined = Some(df);
        }
        Some(previous) => {
            let mut x_series = x_series.clone();
            let mut y_series = y_series.clone();
            let mut z_series = z_series.clone();
            x_series.rename(&format!("X {label}"));
            y_series.rename(&format!("Y {label}"));
            z_series.rename(&format!("Z {label}"));
            let df = DataFrame::new(vec![host_time, x_series, y_series, z_series])?;
            let df = df.sort(
                ["host_time"],
                SortMultipleOptions::default().with_maintain_order(true),
            )?;

            let options = AsOfOptions {
                strategy: AsofStrategy::Backward,
                ..Default::default()
            };

            let new = previous.join(
                &df,
                ["host_time"],
                ["host_time"],
                JoinArgs::new(JoinType::AsOf(options)),
            )?;
            *combined = Some(new);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn plot_combined(
    time: &[f32],
    first: f32,
    last: f32,
    x: &[f32],
    y: &[f32],
    z: &[f32],
    max: f32,
    min: f32,
    red: RGBColor,
    green: RGBColor,
    blue: RGBColor,
    upper: &DrawingArea<BitMapBackend, Shift>,
) -> color_eyre::Result<()> {
    let time_axis = (first..last).step(0.1);
    let mut cc = ChartBuilder::on(upper)
        .margin(10)
        .set_all_label_area_size(50)
        .build_cartesian_2d(time_axis, min..max)?;

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
    Ok(())
}

fn get_ident(input: PathBuf, file_name: &&str) -> color_eyre::Result<(String, String)> {
    let (sensor_tag, ident) = if let Some(index) = file_name.find('-') {
        let sensor_tag = &file_name[..index];
        let file = format!("{sensor_tag}-ident-ident-x64.csv");
        let file = input.join(file);

        let df = CsvReadOptions::default()
            .with_infer_schema_length(Some(10))
            .with_has_header(true)
            .try_into_reader_with_file_path(Some(file.clone()))?
            .finish()?;

        let maker_filter = df.column("code")?.cast(&DataType::String)?.equal("maker")?;
        let prod_filter = df
            .column("code")?
            .cast(&DataType::String)?
            .equal("product")?;

        let _maker = if let Ok(row) = df.filter(&maker_filter)?.column("value")?.get(0) {
            row.get_str().expect("expected string").to_string()
        } else {
            String::new()
        };
        let product = if let Ok(row) = df.filter(&prod_filter)?.column("value") {
            row.get(0)?.get_str().expect("expected string").to_string()
        } else {
            String::new()
        };

        (String::from(sensor_tag), product)
    } else {
        (String::new(), String::new())
    };
    Ok((sensor_tag, ident))
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
