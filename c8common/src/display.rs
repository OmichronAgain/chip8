use crate::pixel::Pixel;
use crate::Datum;

#[derive(Debug, Copy, Clone)]
pub struct Display([[Pixel; 64]; 32]);

impl Display {
    pub fn blank() -> Self {
        Self([[Pixel::Black; 64]; 32])
    }

    pub fn raw(&self) -> &[[Pixel; 64]; 32] {
        &self.0
    }

    pub fn clear(&mut self) {
        self.0 = [[Pixel::Black; 64]; 32];
    }

    pub fn sprite(&mut self, x: Datum, y: Datum, data: &[Datum]) -> ScreenModification {
        let mut modified = ScreenModification::Nothing;
        for (row, byte) in data.iter().enumerate().map(|(i, d)| (i + y.0 as usize, d)) {
            for (column, bit) in Self::split_datum(*byte)
                .into_iter()
                .enumerate()
                .map(|(i, b)| (i + x.0 as usize, b))
            {
                if bit {
                    modified.set();
                    if self.xor_pixel_at(column % 64, row % 32) {
                        modified.clear();
                    }
                }
            }
        }
        modified
    }

    fn pixel_at(&self, x: usize, y: usize) -> &Pixel {
        &self.0[y][x]
    }

    fn pixel_at_mut(&mut self, x: usize, y: usize) -> &mut Pixel {
        &mut self.0[y][x]
    }

    fn set_pixel_at(&mut self, x: usize, y: usize, to: Pixel) -> Pixel {
        let old = *self.pixel_at(x, y);
        *self.pixel_at_mut(x, y) = to;
        old
    }

    fn xor_pixel_at(&mut self, x: usize, y: usize) -> bool {
        if *self.pixel_at(x, y) == Pixel::Black {
            self.set_pixel_at(x, y, Pixel::White);
            false
        } else {
            self.set_pixel_at(x, y, Pixel::Black);
            true
        }
    }

    fn split_datum(datum: Datum) -> [bool; 8] {
        let inner = datum.0;
        let b_bits = [
            inner & 0b10000000,
            inner & 0b01000000,
            inner & 0b00100000,
            inner & 0b00010000,
            inner & 0b00001000,
            inner & 0b00000100,
            inner & 0b00000010,
            inner & 0b00000001,
        ];
        b_bits.map(|x| x != 0)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[must_use]
pub enum ScreenModification {
    Nothing,
    Sets,
    Clears,
}

impl ScreenModification {
    fn set(&mut self) {
        if *self == Self::Nothing {
            *self = Self::Sets;
        }
    }

    fn clear(&mut self) {
        *self = Self::Clears;
    }
}
