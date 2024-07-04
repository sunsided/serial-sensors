use std::fmt::Display;
use std::ops::Neg;

use num_traits::ConstZero;
use ratatui::prelude::*;
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::SensorData;

pub fn axis_to_span<'a, V>(value: V, highlight: bool) -> Span<'a>
where
    V: Display + 'a,
{
    let span = Span::styled(format!("{:+4.6}", value), Style::default());
    if highlight {
        span.green()
    } else {
        span.white()
    }
}

pub fn raw_to_span<'a, V>(value: V, highlight: bool) -> Span<'a>
where
    V: Display + 'a,
{
    let span = Span::styled(format!("{:+4.6}", value), Style::default());
    if highlight {
        span.white()
    } else {
        span.gray()
    }
}

pub fn highlight_axis_3<T>(x: T, y: T, z: T) -> (bool, bool, bool)
where
    T: PartialOrd + ConstZero + Neg<Output = T>,
{
    let x = if x > T::ZERO { x } else { -x };
    let y = if y > T::ZERO { y } else { -y };
    let z = if z > T::ZERO { z } else { -z };

    if x > y && x > z {
        (true, false, false)
    } else if y > x && y > z {
        (false, true, false)
    } else if z > x && z > y {
        (false, false, true)
    } else {
        (false, false, false)
    }
}

pub fn frame_data_to_line(frame: &Version1DataFrame, line: &mut Vec<Span>) {
    match frame.value {
        SensorData::AccelerometerI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("acc", Style::default().cyan()),
                "  = (".into(),
                axis_to_span(vec.x as f32 / 16384.0, highlight_x), // TODO: Don't assume normalization
                ", ".into(),
                axis_to_span(vec.y as f32 / 16384.0, highlight_y), // TODO: Don't assume normalization
                ", ".into(),
                axis_to_span(vec.z as f32 / 16384.0, highlight_z), // TODO: Don't assume normalization
                ")".into(),
            ]);
        }
        SensorData::MagnetometerI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("mag", Style::default().cyan()),
                "  = (".into(),
                axis_to_span(vec.x as f32 / 1100.0, highlight_x), // TODO: Don't assume normalization
                ", ".into(),
                axis_to_span(vec.y as f32 / 1100.0, highlight_y), // TODO: Don't assume normalization
                ", ".into(),
                axis_to_span(vec.z as f32 / 1100.0, highlight_z), // TODO: Don't assume normalization
                ")".into(),
            ]);
        }
        SensorData::TemperatureI16(value) => {
            line.extend(vec![
                Span::styled("temp", Style::default().cyan()),
                " = ".into(),
                axis_to_span(value.value as f32 / 8.0 + 20.0, false), // TODO: Don't assume normalization
                "°C".into(),
            ]);
        }
        _ => {}
    }
}

pub fn frame_data_to_line_raw(frame: &Version1DataFrame, line: &mut Vec<Span>) {
    match frame.value {
        SensorData::AccelerometerI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("acc", Style::default().cyan()),
                "  = (".into(),
                raw_to_span(vec.x, highlight_x),
                ", ".into(),
                raw_to_span(vec.y, highlight_y),
                ", ".into(),
                raw_to_span(vec.z, highlight_z),
                ")".into(),
            ]);
        }
        SensorData::MagnetometerI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("mag", Style::default().cyan()),
                "  = (".into(),
                raw_to_span(vec.x, highlight_x),
                ", ".into(),
                raw_to_span(vec.y, highlight_y),
                ", ".into(),
                raw_to_span(vec.z, highlight_z),
                ")".into(),
            ]);
        }
        SensorData::TemperatureI16(value) => {
            line.extend(vec![
                Span::styled("temp", Style::default().cyan()),
                " = ".into(),
                raw_to_span(value.value, false),
                "°C".into(),
            ]);
        }
        _ => {}
    }
}
