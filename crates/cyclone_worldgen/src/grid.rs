//! Flat 2-D grid with deterministic indexing (spec 02 A4).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid<T> {
    pub width: u32,
    pub height: u32,
    data: Vec<T>,
}

impl<T: Clone> Grid<T> {
    pub fn new(width: u32, height: u32, fill: T) -> Self {
        Self {
            width,
            height,
            data: vec![fill; (width * height) as usize],
        }
    }

    #[inline]
    pub fn idx(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y * self.width + x) as usize)
        } else {
            None
        }
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> Option<&T> {
        self.idx(x, y).map(|i| &self.data[i])
    }

    #[inline]
    pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut T> {
        let i = self.idx(x, y)?;
        Some(&mut self.data[i])
    }

    /// Out-of-bounds sets are silently ignored (keeps generation code terse).
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, v: T) {
        if let Some(i) = self.idx(x, y) {
            self.data[i] = v;
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// (x, y, value) for every cell — always in the same order (A2.5).
    pub fn iter_xy(&self) -> impl Iterator<Item = (u32, u32, &T)> {
        (0..self.height).flat_map(move |y| {
            (0..self.width).map(move |x| (x, y, &self.data[(y * self.width + x) as usize]))
        })
    }

    pub fn fill(&mut self, v: T) {
        self.data.fill(v);
    }
}

impl<T: Copy> Grid<T> {
    /// In-bounds read by value (generation code only indexes in-bounds).
    #[inline]
    pub fn raw(&self, x: u32, y: u32) -> T {
        self.data[(y * self.width + x) as usize]
    }
}
