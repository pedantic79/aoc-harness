use std::ops::{Index, IndexMut};

use num::Integer;

#[derive(Debug, Clone)]
pub struct Grid2d<T> {
    data: Vec<T>,
    size: Coord,
}

type Coord = (usize, usize);
impl<T: Copy> Grid2d<T> {
    pub fn from_elem(size: Coord, elem: T) -> Self {
        Self {
            data: vec![elem; size.0 * size.1],
            size,
        }
    }
}
impl<T> Index<Coord> for Grid2d<T> {
    type Output = T;

    fn index(&self, index: Coord) -> &Self::Output {
        &self.data[index.0 * self.size.1 + index.1]
    }
}
impl<T> IndexMut<Coord> for Grid2d<T> {
    fn index_mut(&mut self, index: Coord) -> &mut Self::Output {
        &mut self.data[index.0 * self.size.1 + index.1]
    }
}
impl<T> Grid2d<T> {
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn get(&self, p: Coord) -> Option<&T> {
        if p.0 < self.size.0 && p.1 < self.size.1 {
            Some(&self[p])
        } else {
            None
        }
    }
    pub fn dim(&self) -> Coord {
        self.size
    }
    pub fn indexed_iter(&self) -> impl Iterator<Item = (Coord, &T)> {
        self.data
            .iter()
            .enumerate()
            .map(|(x, v)| (x.div_mod_floor(&self.size.1), v))
    }
    pub fn neighbours(&self, p: Coord) -> impl Iterator<Item = (Coord, &T)> {
        [
            (p.0.wrapping_sub(1), p.1),
            (p.0, p.1.wrapping_sub(1)),
            (p.0 + 1, p.1),
            (p.0, p.1 + 1),
        ]
        .into_iter()
        .filter_map(|x| self.get(x).map(|v| (x, v)))
    }
    pub fn from_str<F>(input: &str, conv: F) -> Self
    where
        F: Fn(char) -> T,
    {
        let mut stride = None;
        let mut data = Vec::with_capacity(input.len());
        let mut rows = 0;
        for l in input.lines() {
            rows += 1;
            match (stride, l.len()) {
                (None, l) => stride = Some(l),
                (Some(a), b) if a != b => panic!("Not equal line lengths"),
                _ => {}
            }
            for c in l.chars() {
                data.push(conv(c))
            }
        }
        Self {
            data,
            size: (rows, stride.unwrap()),
        }
    }
}