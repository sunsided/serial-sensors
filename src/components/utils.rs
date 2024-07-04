use std::fmt::Display;
use std::ops::Neg;

use num_traits::ConstZero;
use ratatui::prelude::*;

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
