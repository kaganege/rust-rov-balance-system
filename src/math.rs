use core::ops::{Add, Div, Mul, Sub};

pub fn map_range<T: Copy>(s: T, from_range: (T, T), to_range: (T, T)) -> T
where
  T: Add<T, Output = T> + Sub<T, Output = T> + Mul<T, Output = T> + Div<T, Output = T>,
{
  to_range.0 + (s - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
}
